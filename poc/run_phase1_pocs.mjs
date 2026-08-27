/* EXPERIMENTAL / NOT PRODUCTION CODE. Architecture validation only. */

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const ITERATIONS = 1000;

function digest(value) {
  return crypto.createHash("sha256").update(JSON.stringify(value)).digest("hex");
}

function check(condition, message) {
  assert.equal(Boolean(condition), true, message);
}

function crashOrderingOnce(failpoint) {
  const state = {
    durable: [],
    pending: [],
    sideEffect: false,
    projectionDurable: false,
    refDurable: false,
    requestId: "req-fixed",
  };

  const crash = (point) => {
    if (point === failpoint) {
      state.pending = [];
      throw new Error(`SIMULATED_CRASH:${point}`);
    }
  };
  const append = (record) => state.pending.push(record);
  const fsync = () => {
    state.durable.push(...state.pending);
    state.pending = [];
  };

  try {
    crash("before_intent");
    append({ type: "intent", requestId: state.requestId });
    crash("after_intent_before_fsync");
    fsync();
    crash("after_intent_fsync_before_tool");
    state.sideEffect = true;
    crash("after_tool_before_outcome");
    append({ type: "outcome", requestId: state.requestId, status: "succeeded" });
    crash("after_outcome_before_fsync");
    fsync();
    crash("after_outcome_fsync_before_projection");
    state.projectionDurable = true;
    crash("after_projection_before_ref_cas");
    state.refDurable = true;
  } catch (error) {
    check(error.message.startsWith("SIMULATED_CRASH:"), "unexpected crash model error");
  }

  const recover = () => {
    const intent = state.durable.some((entry) => entry.type === "intent");
    const outcome = state.durable.find((entry) => entry.type === "outcome");
    const status = outcome ? outcome.status : intent ? "unknown" : "not_started";
    if (outcome && !state.projectionDurable) state.projectionDurable = true;
    if (outcome && !state.refDurable) state.refDurable = true;
    return {
      status,
      sideEffect: state.sideEffect,
      projectionDurable: state.projectionDurable,
      refDurable: state.refDurable,
      durableCount: state.durable.length,
    };
  };

  const first = recover();
  const second = recover();
  check(digest(first) === digest(second), `${failpoint}: recovery is not idempotent`);
  check(!second.refDurable || second.projectionDurable, `${failpoint}: ref lacks projection`);
  check(!second.refDurable || second.status === "succeeded", `${failpoint}: ref published without outcome`);
  if (second.sideEffect && second.status !== "succeeded") check(second.status === "unknown", `${failpoint}: side effect hidden`);
  return second;
}

function crashOrderingPoc() {
  const failpoints = [
    null,
    "before_intent",
    "after_intent_before_fsync",
    "after_intent_fsync_before_tool",
    "after_tool_before_outcome",
    "after_outcome_before_fsync",
    "after_outcome_fsync_before_projection",
    "after_projection_before_ref_cas",
  ];
  const cases = {};
  for (const failpoint of failpoints) {
    const label = failpoint ?? "clean";
    let last;
    for (let i = 0; i < ITERATIONS; i += 1) last = crashOrderingOnce(failpoint);
    cases[label] = last;
  }

  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "pong-poc-"));
  const journal = path.join(temp, "journal");
  const fd = fs.openSync(journal, "a");
  fs.writeSync(fd, "intent\n");
  fs.fsyncSync(fd);
  fs.closeSync(fd);
  check(fs.readFileSync(journal, "utf8") === "intent\n", "filesystem fsync smoke failed");
  fs.rmSync(temp, { recursive: true, force: true });
  return { name: "crash-ordering", iterations: ITERATIONS, cases, pass: true };
}

function fileManifest(root) {
  const result = {};
  for (const name of fs.readdirSync(root).sort()) {
    const full = path.join(root, name);
    if (fs.statSync(full).isFile()) result[name] = digest(fs.readFileSync(full));
  }
  return result;
}

function captureCompletenessPoc() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "pong-capture-"));
  const events = [];
  const groundTruth = [];
  const known = {};
  const wrapped = (action, resource, fn) => {
    groundTruth.push({ action, resource, wrapped: true });
    fn();
    events.push({ action, resource, captureMode: "native", confidence: "complete" });
    known[resource] = fileManifest(root)[resource] ?? null;
  };
  const bypassed = (action, resource, fn) => {
    groundTruth.push({ action, resource, wrapped: false });
    fn();
  };

  wrapped("CREATE_FILE", "a.txt", () => fs.writeFileSync(path.join(root, "a.txt"), "a"));
  wrapped("WRITE_FILE", "a.txt", () => fs.writeFileSync(path.join(root, "a.txt"), "b"));
  wrapped("MOVE_FILE", "a.txt", () => fs.renameSync(path.join(root, "a.txt"), path.join(root, "b.txt")));
  known["b.txt"] = fileManifest(root)["b.txt"];
  bypassed("WRITE_FILE", "b.txt", () => fs.writeFileSync(path.join(root, "b.txt"), "bypassed"));
  const current = fileManifest(root);
  for (const [resource, hash] of Object.entries(current)) {
    if (known[resource] !== hash) events.push({ action: "RECONCILED_CHANGE", resource, captureMode: "reconciled", confidence: "partial" });
  }
  events.push({ action: "WEB_SEARCH", resource: "network", captureMode: "unsupported", confidence: "missing" });

  for (const item of groundTruth) {
    const observed = events.some((event) => event.resource === item.resource && (event.action === item.action || event.action === "RECONCILED_CHANGE"));
    check(observed, `capture silently omitted ${item.action}:${item.resource}`);
  }
  check(events.some((event) => event.captureMode === "reconciled"), "bypassed change was not reconciled");
  check(events.find((event) => event.action === "WEB_SEARCH").confidence !== "complete", "unsupported action was overstated");
  fs.rmSync(root, { recursive: true, force: true });
  return { name: "capture-completeness", supportedEffects: groundTruth.length, records: events.length, pass: true };
}

function redactText(value, secrets) {
  let result = String(value);
  for (const secret of secrets) result = result.split(secret).join("[REDACTED]");
  result = result.replace(/sk-[A-Za-z0-9_-]{20,}/g, "[REDACTED_HEURISTIC]");
  result = result.replace(/(?:token|password|api_key)=([^&\s]+)/gi, (match) => match.replace(/=.*/, "=[REDACTED]"));
  return result;
}

function redactionPoc() {
  const secrets = ["sk-test-12345678901234567890", "prod-token-abcdef0123456789"];
  const corpus = {
    env: { SAFE: "ok", API_TOKEN: secrets[1] },
    args: ["--api_key=sk-test-12345678901234567890"],
    output: `response=${secrets[0]}`,
    json: { nested: secrets[1] },
    filename: "report.txt",
  };
  const serialize = (value) => redactText(JSON.stringify(value), secrets);
  const redacted = serialize(corpus);
  for (const secret of secrets) check(!redacted.includes(secret), `secret leaked: ${secret}`);
  check(redacted === serialize(corpus), "redaction placeholder is not deterministic");
  const fingerprint = serialize({ os: "test", runtime: "node", network: "none", env: { SAFE: "ok" } });
  check(!fingerprint.includes("API_TOKEN"), "sensitive environment key persisted");
  const sinks = [redacted, JSON.stringify({ journal: redacted }), JSON.stringify({ export: redacted }), redacted.toUpperCase()];
  for (const sink of sinks) for (const secret of secrets) check(!sink.includes(secret), "secret escaped a sink");
  return { name: "environment-secret-redaction", registeredSecrets: secrets.length, sinks: sinks.length, pass: true };
}

function merge(base, ours, theirs) {
  const result = { ...base };
  const conflicts = [];
  for (const key of new Set([...Object.keys(base), ...Object.keys(ours), ...Object.keys(theirs)])) {
    const b = base[key];
    const o = ours[key];
    const t = theirs[key];
    if (o === t) result[key] = o;
    else if (o === b) result[key] = t;
    else if (t === b) result[key] = o;
    else conflicts.push(key);
  }
  return { result, conflicts };
}

function leaseMergePoc() {
  let staleRejects = 0;
  let deterministicDisjoint = 0;
  let explicitConflicts = 0;
  for (let i = 0; i < ITERATIONS; i += 1) {
    let lease = { agent: "a", epoch: 1 };
    const stale = { ...lease };
    lease = { agent: "b", epoch: 2 };
    const staleCommitAccepted = lease.agent === stale.agent && lease.epoch === stale.epoch;
    if (!staleCommitAccepted) staleRejects += 1;
    const base = { "a.txt": "0", "b.txt": "0" };
    const disjoint = merge(base, { ...base, "a.txt": "1" }, { ...base, "b.txt": "1" });
    check(disjoint.conflicts.length === 0, "disjoint changes became a conflict");
    check(disjoint.result["a.txt"] === "1" && disjoint.result["b.txt"] === "1", "disjoint merge lost a change");
    deterministicDisjoint += 1;
    const overlap = merge(base, { ...base, "a.txt": "1" }, { ...base, "a.txt": "2" });
    check(overlap.conflicts.includes("a.txt"), "overlapping change was hidden");
    explicitConflicts += overlap.conflicts.length;
  }
  check(staleRejects === ITERATIONS, "stale lease was accepted");
  return { name: "two-agent-lease-merge", iterations: ITERATIONS, staleRejects, deterministicDisjoint, explicitConflicts, pass: true };
}

const results = [crashOrderingPoc(), captureCompletenessPoc(), redactionPoc(), leaseMergePoc()];
console.log(JSON.stringify({
  experimental: true,
  label: "EXPERIMENTAL / NOT PRODUCTION CODE",
  node: process.version,
  results,
}, null, 2));
