# Next Task

**Current Phase:** M4 - provider-neutral external protocol boundary

**Current Milestone:** External Agent Protocol and local process transport

**M1 release baseline:** `v0.1.0` at
`2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`; its tag and release evidence are
immutable.

**Current Task:** Pong Core is frozen as an offline-first, local-first,
network-independent execution/state/control plane. M4-007 verified the complete
local durable workflow without network, HTTP, MCP, cloud services, remote DB, or
external authentication. Remote access remains an optional adapter.

The External Agent Protocol v1.0 remains frozen as the provider-neutral
control contract. M4-005 evaluated JSONL, CLI, HTTP, WebSocket, and MCP against
Pong's control-plane requirements. JSONL remains the development/local
transport; MCP is an optional adapter through the protocol, never a Core
interface. Production authentication and remote transports remain deferred.

The prior transport description remains applicable: Protocol version `1.0` now has a minimal development-only JSON
Lines process adapter. `pong-agent-protocol` maps stdin/stdout messages to the
transport-neutral dispatcher, supplies host lease time, and resolves constrained
single-component Workspace bindings beneath a host-owned root.

M4-004 lifecycle/reconnect hardening is now implemented and test-gated:
durable generic Operations can be started and finished, resolved by request
identity after a lost response, inspected after cold reopen, and retried with
existing Operation idempotency. Request, Operation, Execution, Agent, and
transport-session identities remain separate.

**Contract materials:** [`M4_EXTERNAL_AGENT_PROTOCOL.md`](../architecture/M4_EXTERNAL_AGENT_PROTOCOL.md),
[`ADR-M4-external-agent-protocol.md`](../decisions/ADR-M4-external-agent-protocol.md),
and [`external_agent_protocol.rs`](../../tests/external_agent_protocol.rs).

**Checkpoint:** The external protocol and local JSON Lines transport are
`IMPLEMENTED / INTERNAL / TEST-GATED`. Two process tests cover malformed input,
binding traversal rejection, complete Runtime A to Runtime B continuation, and
reconnect through a fresh process. Existing protocol, AgentControl, and M3
rollback suites remain regression gates. Ignored tests are never counted as
passes. MCP, HTTP, SDK adapters, remote execution, and production authentication
remain `NOT_PROVEN`.

**Next Action:** Specify remote synchronization and transport requirements only
after the offline Core and security boundaries are reviewed. Do not make HTTP,
MCP, provider-specific behavior, scheduling, orchestration, or network services
Core dependencies.

## M1 Historical Handoff

The minimal native-platform workflow is now defined at
`.github/workflows/m1-release-evidence.yml`. Runs `33074865773` through
`33142438624` are retained historical failures. Follow-up run `33145714975`
at commit `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` completed both native
jobs successfully; all 13 commands per platform returned zero and the
artifacts remain retained predecessor evidence. Final candidate CI Run
`33253638226` at binding commit `b20fc903...` also completed both required
GitHub-hosted jobs successfully; Windows has no GitHub Actions requirement.

The workflow emits stable/MSRV logs, per-command exit codes, cold-reopen output,
workspace-local test-repository filesystem metadata, an artifact manifest, and
SHA256SUMS under the named native evidence artifacts. Run `33074865773` exposed
two real defects: Linux evidence hashes were invalidated by checkout line-ending
normalization, while macOS compiled a platform-inapplicable host-fault test.
Run `33080915116` was retained after the first correction: Linux still failed
both full tests in `artifact_consistency`, and macOS produced an incomplete
artifact while its MSRV test path failed (MSRV clippy exited zero). Both
failures are retained.
The earlier bundle-wide byte-preservation correction is pushed at `96a2f8c`;
workflow capture-boundary hardening is pushed at
`2cf136e99091d8d074d1a0e597f72f06bd2d52f7`. This close-out additionally
widens the Git byte boundary to all `artifacts/**` and fixes the PT-13/FI-10
release-log reference; the successful run is now retained and indexed. Follow
[`M1_CI_EXECUTION_REQUIRED.md`](../development/M1_CI_EXECUTION_REQUIRED.md)
for the external execution handoff.

**Blocked By:** No coding, testing, or evidence blocker remains for the
accepted M1 scope. The source candidate is `58e9e875...`, final CI is Run
`33253638226`, and the retained bundle has been checksummed. The normal
owner-controlled release tag is intentionally not created in this task. M2/M3
remain internal test-gated slices and do not waive M1.

**Next Action:** Release Owner reviews the frozen candidate and Run
`33253638226` evidence, then uses the normal controlled process to create and
push the release tag. Review [`M1_RELEASE_SCOPE_DECISION.md`](../development/M1_RELEASE_SCOPE_DECISION.md),
[`M1_FAULT_SCOPE_DECISION.md`](../development/M1_FAULT_SCOPE_DECISION.md),
[`M1_PROPERTY_SCOPE_DECISION.md`](../development/M1_PROPERTY_SCOPE_DECISION.md),
[`M1_PERFORMANCE_SCOPE_DECISION.md`](../development/M1_PERFORMANCE_SCOPE_DECISION.md),
[`M1_PROJECTION_ACCEPTANCE.md`](../development/M1_PROJECTION_ACCEPTANCE.md),
and [`M1_OWNER_ACTION_LIST.md`](../development/M1_OWNER_ACTION_LIST.md).
Final-CI Run `33253638226` and the reconciled artifact bundle are complete.
The Owner's only remaining action is the normal controlled release/tag process;
do not create a tag or enter M2 from this task.

Run `33142438624` at commit `e96131d9bb6d3799055401841d0cb710e4f497ff` is
retained as the fourth native failure record. Both complete artifacts show
stable/MSRV fmt/check/clippy, focused M1 suites, and cold reopen passing, but
both full test commands exited `101` in `artifact_consistency`: the committed
LF `build-metadata.json` differed from stale CRLF hash/reference values. The
corrected committed digest is now recorded in the bundle; push and dispatch
again before accepting either native row.

Run `33085292318` at commit `60913c0179731d9fed1a052f9a190c8fa4f5a56e` is
retained as the third native failure record. Both Linux and macOS artifacts
are complete; stable/MSRV fmt/check/clippy, focused M1 suites, and cold reopen
passed, but both full test commands exited `101` in `artifact_consistency`
because committed LF evidence bytes differed from the references used by that
run. The macOS runner reports filesystem metadata as `unknown`. The artifact
ZIP hashes are retained in the run download metadata. The repository-wide
`artifacts/** -text -diff` rule and explicit release-log reference now fix the
byte boundary; these changes are committed and pushed in
`258a0c9cccfe11e994a6032a106644b4dcf90804`. A fresh native dispatch remains
required after the checksum/reference correction.

The retained Windows stable property evidence includes PT-01/02/03/04/05/06/
07/08/11/12 at 10,000 cases each, PT-09 and PT-10 at 10,000 cases each, and the
PT-14 normative rerun at 10,008 executed cases across nine migration failpoints:
[`windows-stable-pt14-10000-rerun-2026-08-27.json`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json).
The PT-14 bounded concurrent diagnostics remain supplemental:
[`windows-stable-pt14-diagnostic-2026-08-27.json`](../../artifacts/m1-property-runs/windows-stable-pt14-diagnostic-2026-08-27.json).
Normative PT-14 runs on other accepted rows are still required. Historical
no-result records remain retained to show the audit boundary.

The continuation audit also added file-backed cold-reopen evidence for
synthetic FI-01/FI-08, the foreign-valid-generation SQLite replacement
regression, read-only artifact consistency checks, and a pinned Docker-VM ext4
run with `TMPDIR` on the ext4 volume. These strengthen local evidence but do
not satisfy the external platform, old-reader, fault, budget, or
release-owner requirements.

The 2026-08-27 close-out reran the complete Windows stable Rust 1.95.0 and
Rust 1.78.0 MSRV gates in independent target directories; every command
returned exit code `0`. Git traceability is available (`dev` at
`6cb62fb455e92ab731a4bb5233856d10c1f1ce93`, remote `origin`); the regenerated
traceability record correctly reports the current evidence tree as dirty, with
no release tag or owner-approved release commit. The retained executable
evidence is bound to native workflow `33145714975` at commit `6cb62fb` with
exact commands, toolchain identity, test results, and artifact hashes. Native
follow-up run `33145714975` at commit `6cb62fb` completed
both jobs successfully with all 13 commands per platform returning `0`; its
complete artifacts are retained and native evidence rows are executable
`PASS`. M1 remains blocked by the old-reader, complete fault/property scope,
performance budget, and release-owner decisions.

**Definition of Done:** Every M1 MUST-PASS row has platform-specific executable
evidence or an explicitly accepted ADR disposition. For the accepted bounded
scope, the technical gate and final CI are complete; the release tag remains a
separate owner-controlled action.

## Session rule

Every future session starts here. If the next action changes, update this file
before changing implementation or roadmap scope.
