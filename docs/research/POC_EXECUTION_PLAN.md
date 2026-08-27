# Phase 1 PoC Execution Plan

## Purpose

This plan turns the four Phase 1 research gates into repeatable experiments. The goal is to validate the assumptions behind M1 durable primitives, runtime capture, security redaction, and optimistic collaboration before production implementation starts.

These are architecture-validation PoCs, not production benchmarks and not a claim that Pong is a general-purpose sandbox. Every result must state the tested platform, driver, runtime, and capture method.

## Scope and shared rules

The PoCs share one disposable local repository fixture with:

- deterministic IDs, clock, scheduler, and random seed;
- a temporary CAS, metadata store, append-only journal, event projection, refs, and workspace;
- an operation request ID and idempotency key on every attempted operation;
- a fault-injection switch at each documented durability or capture boundary;
- a ground-truth trace independent of the Pong record being tested;
- machine-readable raw results plus a short human-readable report.

The fixture must preserve failed runs for diagnosis, but test secrets and host-specific paths must be sanitized before results are committed. Each PoC runs at least 1,000 repetitions per failpoint or interleaving where the harness can do so deterministically; smaller samples require an explicit reason in the report.

### Common pass conditions

All PoCs must preserve these invariants:

1. No committed event, ref, snapshot, or artifact points to a missing or corrupt object.
2. Re-running recovery or projection is idempotent.
3. Every uncertain effect is visible as `unknown`, `unreconciled`, or an explicit conflict; it is never silently treated as success or safe retry.
4. Request IDs and event IDs do not produce duplicate semantic effects when a caller retries.
5. Findings that change a documented architecture assumption produce an ADR before the next milestone gate.

## Execution order

| Order | PoC | Primary question | Why this order | Gate |
| --- | --- | --- | --- | --- |
| 1 | Crash ordering and recovery | Does the journal preserve truth across every crash window? | All later PoCs rely on durable intent, outcome, projection, and ref semantics. | Blocks all implementation if any safety invariant fails. |
| 2 | Capture completeness | Which supported actions are actually observed and attributed? | Redaction and collaboration tests need a measured capture surface and confidence vocabulary. | Defines the v0.1 support matrix and reconciliation gaps. |
| 3 | Environment and secret redaction | Can captured facts be persisted/exported without leaking registered secrets? | Uses the real capture envelopes and payload shapes established by PoC 2. | Blocks release claims if a registered secret can escape. |
| 4 | Two-agent lease and merge | Are concurrent writes isolated, detected, and merged without silent loss? | Exercises the durable events, operation provenance, and workspace semantics validated above. | Blocks collaboration milestone if stale or conflicting writes are hidden. |

Do not optimize or broaden the implementation between gates. If a PoC fails, record the failure, update the relevant ADR or open question, and rerun the smallest affected experiment before proceeding.

## PoC 1: Crash ordering and recovery

**Owner:** Reliability / Storage  
**Related decisions:** ADR-0007, ADR-0008, ADR-0012; OQ-001, OQ-007

### Assumptions under test

- A mutating operation writes a durable intent before invocation.
- Outcome and state-delta evidence are durable before publication to the event index and ref transaction.
- An intent without a durable outcome is recovered as `unknown`, not as success or failure.
- Journal replay, event projection, and ref updates are idempotent.

### Experiment design

Run a deterministic mutating fixture through each boundary and terminate the process at:

1. before intent append;
2. after intent append but before intent fsync;
3. after intent fsync and before tool invocation;
4. after tool side effect but before outcome append;
5. after outcome append but before outcome fsync;
6. after outcome fsync but before event projection;
7. after projection but before ref CAS;
8. during ref CAS and object publication.

Repeat with clean completion, retry of the same request ID, disk-full simulation, and a corrupt or truncated journal tail. On restart, run recovery twice and compare repository state, reachable objects, refs, and event projections.

### Observed metrics

- committed-event loss and duplicate semantic effects;
- classification accuracy for `unknown` and `unreconciled` operations;
- refs pointing to missing, unpublished, or corrupt objects;
- projection and recovery idempotence;
- count and visibility of pending reconciliation items;
- recovery wall time and journal scan throughput;
- retry behavior for the same request ID.

### Pass / fail thresholds

- **Pass:** zero silent loss, duplicate semantic effects, invalid refs, or corruption across all repetitions and failpoints; every durable orphan intent is surfaced; the second recovery produces byte-for-byte equivalent logical state; same-key retry is idempotent.
- **Fail:** any committed fact disappears, any uncertain effect is reported as a definite success/failure, any invalid ref becomes visible, or recovery changes state on its second run.
- Performance is informative for this gate. Report p50/p95 and compare with a clean-start baseline; a regression greater than 2x requires investigation, but is not a safety pass by itself.

### Risks and mitigations

- A kill signal may not model power loss; include fsync-aware storage tests and document the limitation.
- Filesystem or database behavior may differ by host; record OS/filesystem and repeat on each supported platform.
- A tool can complete after the runtime is killed; use a side-channel ground truth marker and preserve `unknown` semantics.

### Outputs and ADR routing

Produce `docs/research/results/POC-001-crash-ordering.md`, failpoint coverage, raw run summaries, and a recovery state table. An ADR is required if ordering, `unknown` semantics, durability boundaries, or idempotency rules differ from ADR-0007/0008/0012; otherwise update those ADRs with evidence and close OQ-001/OQ-007.

## PoC 2: Capture completeness

**Owner:** Runtime / Adapter  
**Related decisions:** ADR-0005, ADR-0009; OQ-001, OQ-004

### Assumptions under test

- Layered capture (explicit API, process instrumentation, filesystem instrumentation, and adapters) can cover the declared v0.1 support matrix.
- Each record carries capture method and confidence; unsupported or partially observed actions are not presented as complete.
- Reconciliation can attribute unobserved filesystem changes without inventing tool intent.

### Experiment design

Build a corpus of actions with an independent ground-truth trace: create, write, append, rename, delete, chmod, symlink, subprocess, shell pipeline, timeout, cancellation, and child-process file mutation. Run each action through each supported capture layer, then combine layers as v0.1 would. Include deliberately bypassed wrappers, abrupt process death, concurrent writers, and unsupported network/database/browser actions to verify gap labeling.

Compare the ground-truth trace with operation envelopes, event records, final manifests, and reconciliation output. Run on every declared initial platform and runtime host.

### Observed metrics

- event recall by action type and capture layer;
- intent/outcome pairing rate;
- attribution precision (which operation caused which change);
- duplicate event rate and ordering skew;
- time from effect to durable record;
- proportion of gaps labeled `unreconciled` or low confidence;
- false claims of complete capture for unsupported actions.

### Pass / fail thresholds

- **Pass for supported v0.1 actions:** 100% of controlled effects have a corresponding durable record or explicit reconciliation record; 100% have a method and confidence label; zero silent omissions; duplicate semantic records are zero after idempotent projection.
- **Pass for unsupported actions:** they are either blocked by policy or visibly marked unsupported/low-confidence; no test may report complete capture.
- **Fail:** one silently missing supported effect, one misattributed effect that could alter rollback/audit meaning, or any unsupported action represented as fully captured.
- Report recall, precision, and latency distributions by platform; do not collapse unsupported coverage into the supported score.

### Risks and mitigations

- Watchers can coalesce or reorder events; retain syscall/process evidence where possible and test reconciliation.
- Child processes and native framework calls may bypass wrappers; declare adapter-specific gaps rather than expanding generic guarantees.
- Platform-specific metadata (permissions, timestamps, case sensitivity) can create false differences; normalize only through a documented driver policy.

### Outputs and ADR routing

Produce `docs/research/results/POC-002-capture-completeness.md`, a support matrix, capture confidence taxonomy, and gap/reconciliation examples. An ADR is required if the support matrix, confidence semantics, interception boundary, or v0.1 completeness claim changes ADR-0009; otherwise update ADR-0009 and close the relevant part of OQ-001/OQ-004.

## PoC 3: Environment and secret redaction

**Owner:** Security / Privacy  
**Related decisions:** ADR-0011; OQ-002, OQ-005, OQ-006

### Assumptions under test

- Environment fingerprints retain reproducibility-relevant facts without persisting secrets or unstable host identifiers by default.
- Registered secrets are redacted before durable storage, indexing, logs, artifacts, snapshots, and export.
- Heuristic detection is a defense in depth, not the sole security boundary.

### Experiment design

Create a corpus containing registered secrets and non-secrets in environment variables, command arguments, stdin, stdout/stderr, JSON, YAML, URLs, headers, filenames, file contents, binary blobs, Unicode, multiline values, and values split across chunks. Include common token formats and project-specific secret registrations. Exercise capture, journal append, projection, snapshot/artifact creation, error paths, status/log output, and event export. Test redaction before and after serialization/compression and under truncation or retry.

For environment fingerprints, compare stable facts (runtime version, declared dependency digest, normalized OS facts) with volatile or sensitive facts (absolute paths, hostnames, tokens, credentials), and verify policy-driven inclusion.

### Observed metrics

- exact registered-secret leak count across every durable and user-visible sink;
- heuristic detector recall and false-positive rate on the corpus;
- deterministic placeholder stability across repeated captures;
- redaction coverage before serialization and after export/decompression;
- fingerprint stability across equivalent hosts and drift detection for meaningful changes;
- performance and payload-size overhead.

### Pass / fail thresholds

- **Pass:** zero plaintext occurrences of any registered secret in CAS blobs, journal, metadata, events, indexes, snapshots, artifacts, logs, errors, or exports; placeholders are deterministic and do not permit trivial recovery; sensitive environment facts are excluded by default.
- Heuristic target: at least 95% recall and at most 10% false positives on the supplied corpus, with misses explicitly reported as residual risk.
- **Fail:** any registered-secret leak, redaction bypass in a secondary sink, or a fingerprint that exposes a configured sensitive fact.

### Risks and mitigations

- Secrets can be encoded, compressed, split, or transformed; test sink boundaries and maintain a registration API.
- Aggressive patterns can destroy useful debugging data; preserve typed metadata and measure false positives.
- Redaction cannot protect a host already compromised by a privileged process; keep the security-boundary statement explicit.

### Outputs and ADR routing

Produce `docs/research/results/POC-003-redaction.md`, the sanitized corpus definition, sink inventory, detector report, and fingerprint policy table. An ADR is required if storage encryption, key scope, redaction timing, default fingerprint fields, or trust boundaries change ADR-0011; otherwise amend ADR-0011 with evidence and close OQ-002/OQ-006. Retention findings should update OQ-005 or its governing policy.

## PoC 4: Two-agent lease and merge

**Owner:** Collaboration / Workspace  
**Related decisions:** ADR-0003, ADR-0004, ADR-0010; OQ-003

### Assumptions under test

- The default workspace has one writable lease; independent workspaces and branches permit safe parallel work.
- Lease, branch-head, and commit updates use optimistic expected-version checks.
- Disjoint file changes can merge deterministically; overlapping or non-file resource changes become explicit conflicts.
- Stale agents cannot publish over a newer head or silently lose another agent's work.

### Experiment design

Use a deterministic two-agent scheduler with lease acquire, renew, expiry, crash, reconnect, snapshot, commit, and merge operations. Cover:

1. disjoint file edits;
2. same-file overlapping edits;
3. rename/delete versus edit;
4. simultaneous commit against one head;
5. stale lease renewal and stale commit;
6. owner crash followed by lease expiry and takeover;
7. conflicting non-file resource references;
8. duplicate retry of acquire, commit, and merge requests.

Run thousands of seeded interleavings and repeat selected schedules with different timing. Compare final trees, refs, conflict records, event provenance, and lease history with the scheduler's ground truth.

### Observed metrics

- simultaneous writable lease violations;
- stale commit/renewal acceptance rate;
- lost-write and silent last-writer-wins count;
- deterministic merge rate for disjoint changes;
- explicit conflict rate and conflict attribution for overlapping/non-file changes;
- takeover time after owner crash;
- idempotence of retried requests and completeness of provenance.

### Pass / fail thresholds

- **Pass:** zero simultaneous default writable leases, stale publications, lost writes, or silent conflict resolution across all seeded schedules; 100% of disjoint file merges are deterministic; every overlap is an explicit conflict or policy-approved deterministic result; retries are idempotent.
- **Fail:** any stale head is accepted, any agent's committed change disappears, or a conflict is hidden from status/log/provenance.
- Report merge latency and conflict-resolution effort as usability evidence, not as a substitute for safety.

### Risks and mitigations

- Clock skew can break lease expiry; use monotonic leases and document wall-clock display separately.
- File-only merges may imply false generality; keep database/browser/remote resources in an explicit conflict policy.
- Crash during merge can leave partial outputs; reuse PoC 1 recovery fixtures and require a safety snapshot before destructive restore/merge steps.

### Outputs and ADR routing

Produce `docs/research/results/POC-004-lease-merge.md`, seeded schedules, support/conflict matrix, and a lease state diagram. An ADR is required if lease ownership, expected-version checks, merge determinism, or non-file conflict policy differs from ADR-0010/0004; otherwise update those ADRs with evidence and close OQ-003.

## Gate review and decision record

At the end of each PoC, publish a report with: commit/repository revision, platform, fixture version, seeds, raw-result location, pass/fail decision, known gaps, and proposed ADR or deferral. The architect and the relevant owner sign the gate. A failed gate does not justify silently weakening thresholds; it requires one of:

- an implementation or fixture correction followed by a rerun;
- a new or amended ADR that narrows the product claim;
- an explicit deferred scope decision with an owner and revisit milestone.

Phase 1 is ready to enter M1 implementation only when PoC 1 passes, the supported capture matrix is accepted, registered-secret redaction passes, and the lease/merge policy is either passing or explicitly deferred with a safe default.
