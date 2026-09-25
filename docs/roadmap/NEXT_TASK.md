# Next Task

**Current Phase:** M4 - provider-neutral external protocol boundary

**Current Milestone:** Cross-platform / native runtime regression gate (M4-020)

**Current Task:** M4-020 verifies that the frozen Core-owned contract is
reproducible on native Linux and macOS: Core ownership, lifecycle, durable
state, lease/revision/CAS, Operation recovery, Protocol v1.0, HTTP remote
transport, authorization, reconnect, restart, and failure semantics. The
first real native workflow run (`36168364866`) passed format/check/clippy but
failed the focused and full regression on both Ubuntu 24.04 and macOS 14:
Linux exposed a temporary-fixture lifetime bug, while macOS exposed the
hosted-runner `/var` symlink ancestry. Windows remains PASS; Linux and macOS
are real `FAIL` evidence, so no M4-020 checkpoint is created. See
[`M4_CROSS_PLATFORM_NATIVE_RUNTIME.md`](../architecture/M4_CROSS_PLATFORM_NATIVE_RUNTIME.md)
and
[`ADR-M4-020-cross-platform-native-runtime.md`](../decisions/ADR-M4-020-cross-platform-native-runtime.md).

**Next Action:** Push the remediation for the Linux cold-reopen fixture and
macOS physical `TMPDIR`, then execute
`.github/workflows/m4-cross-platform-regression.yml` again on both native
runners. Retain complete artifacts and compare semantic outcomes with the
Windows matrix. The failed run record is retained at
[`m4-020-github-actions-run-36168364866.json`](../../artifacts/m4-development/m4-020-github-actions-run-36168364866.json);
the Docker diagnostic remains at
[`m4-020-docker-linux-probe-2026-09-24.json`](../../artifacts/m4-development/m4-020-docker-linux-probe-2026-09-24.json).
Do not promote Linux/macOS to PASS until a native rerun returns zero for every
required command.

**M1 release baseline:** `v0.1.0` at
`2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`; its tag and release evidence are
immutable.

**Prior task:** M4-019 restored the Windows regression gate without changing
the single-Core ownership architecture. Model A direct Repository access
remains `NOT_SUPPORTED`.

**Prior task:** M4-018 froze the existing single trusted collaboration
domain: registered Agents share Task, detailed Workspace, Version, Checkpoint,
Handoff and diff observation, while Execution/Operation visibility and
control remain Agent-scoped. No per-project tenant isolation or Workspace
creator/owner is claimed. Cross-Agent reads expose relative diff file names
and lease metadata, not physical Workspace locator or file contents. HTTP
and JSONL use the same Protocol authorization, with different ingress trust
boundaries. See
[`M4_RESOURCE_AUTHORIZATION.md`](../architecture/M4_RESOURCE_AUTHORIZATION.md)
and
[`ADR-M4-018-resource-authorization.md`](../decisions/ADR-M4-018-resource-authorization.md).
Focused authorization and adjacent matrices pass, but full regression remains
`PARTIAL` on Windows (`STATUS_ACCESS_VIOLATION` in one run, and the existing
direct-handle concurrency test's raw OS codes 33/2 in subsequent runs).
No M4-018 checkpoint is created until the full regression gate is resolved;
do not weaken, serialize or ignore the failing test to force a green result.

**Prior task:** M4-017 inventories Core, AgentControl, Protocol v1.0, JSONL,
HTTP, and Runtime capabilities. Rollback remains Core-owned/internal with
external exposure deferred; arbitrary-destination Restore is internal;
Version materialization, read-only diff, guarded lease control, and durable
Operation reconciliation retain their existing external shape. No command or
Core schema is changed.

The normal external workflow is already proven through independent HTTP
client/server processes. However, Protocol v1.0 permits any registered Agent
to read several resource records and perform compatible Workspace diffs by
ID, without a project-membership or resource-owner read grant. Execution and
Operation writes remain ownership/lease/revision guarded. The broad OBSERVE
scope was an explicit `PARTIAL` authorization gate at M4-017; M4-018 now
freezes trusted-domain sharing without claiming tenant isolation. M4-017's
conditional checkpoint remains absent.
See [`M4_EXTERNAL_CAPABILITY_SURFACE.md`](../architecture/M4_EXTERNAL_CAPABILITY_SURFACE.md)
and [`ADR-M4-017-external-capability-surface.md`](../decisions/ADR-M4-017-external-capability-surface.md).

**Prior task:** M4-015 proves the M4-014 adapter through independent OS
processes without changing the frozen External Agent Protocol v1.0. Separate
`pong-agent-http-client` processes use real loopback TCP to reach an independent
`pong-agent-http` process, then pass unchanged envelopes through
`RemoteAccessBoundary`, `ProtocolDispatch`, AgentControl, and the single Core
owner.

The contract keeps Connection, Session, Principal, Agent, Execution, and
Operation identities distinct. Disconnect, timeout, client restart, and Core
restart do not mutate durable Pong state. Reconnect requires re-authentication,
a new session binding, durable inspection/resolution, and explicit retry or
continuation.

M4-012 passes 18 active Remote Contract tests and a 53-test focused matrix with
no failures or ignored tests. The combined full regression remains
`PARTIAL / ENVIRONMENT` because the pre-existing independent-handle
`control_layer` diagnostic can still receive Windows lock violation code 33;
an unchanged focused rerun passed. This does not promote that unsupported
external access mode or claim a real remote network deployment.

M4-013 adds a development HTTP binary and a synchronous fixed-worker adapter.
It defaults to loopback, requires authentication, reads credentials from an
external strict JSON source, rejects implicit non-loopback binding, and keeps
HTTP status separate from protocol error codes. Sixteen active real TCP tests
cover startup, authentication, authorization, JSONL equivalence, retry,
uncertain outcome, reconnect, Core restart, multi-client access,
lease/revision conflicts, graceful shutdown, and a complete
Checkpoint/Handoff/Resume continuation.

M4-014 adds bounded request and response bodies, configurable fixed protocol
workers, an in-memory per-principal limiter, panic isolation, safe errors,
runtime correlation/metrics, strict credential-file validation, and atomic
credential reload. A real independent HTTP process proves credential A -> B
rotation and A revocation without disturbing Core ownership or durable state.

M4-015 adds eight active process-gated tests using temporary Repository,
Workspace, and credential directories plus OS-selected loopback ports. The
suite proves authentication, authorization, failure isolation, resource
limits, retry, revision refresh, lease conflict, multi-client access, graceful
shutdown, forced Core restart, second-Core rejection, credential rotation, and
the complete Version/Checkpoint/Handoff/Resume continuation. A separate HTTP
and JSONL process scenario compares success, failure, conflicts, missing
Operation, exact retry, and reconnect semantics.

Rollback remains `NOT_PROVEN / PROTOCOL CAPABILITY GAP`: External Agent
Protocol v1.0 has no rollback operation, and this checkpoint does not change
the frozen contract to expose the internal Core capability.

M4-016 freezes transport, Protocol, durable Operation, and Core failure
semantics. It adds a bounded 64-entry secret-free diagnostic ring, bounded
rate-limiter principal tracking, and best-effort shutdown diagnostics without
changing Protocol v1.0 or Core schema. Operation failure remains a Protocol
result rather than HTTP 500; disconnect remains unknown/recovery-required
rather than cancellation.

The `tiny_http` connection parser still uses a dynamically growing internal
task pool and exposes no configurable socket read/idle deadline. Global
connection/queue bounds and slow-client protection remain `NOT_PROVEN` and are
mandatory production-ingress responsibilities.

The production JSONL adapter remains a single attached stdin/stdout stream. It
is not relabeled as a multi-client daemon. Protocol v1.0 and the durable
Operation schema remain unchanged.

The M4-011 focused policy gate passes on Windows. Two real direct-access probe
processes are rejected with `CONFLICT` while a Core owner is active, and the
Repository cold-reopens unchanged after owner shutdown. The M4-008 direct
writer diagnostic now identifies `OPEN` versus `SNAPSHOT` failure and verifies
cold-reopen Workspace heads before reporting an unexpected OS result. Its
accepted result set was not widened.

Windows raw OS error `5` remains `NOT_PROVEN`: it may be an ACL denial or an
access/share race, and Pong has native evidence for the ACL meaning. It is not
globally reclassified as a conflict. Thirty diagnostic reruns did not reproduce
code `5`; they observed successful publication, open-time code `33`, and one
open-time code `2`.

The final M4-011 full regression passed with all active tests green. The
M4-009/010/011 focused matrix passed `137` tests with no failures or ignored
tests. This establishes release readiness for the supported Core-owned external
Runtime path without promoting direct multi-process external access.

M4-008 concurrency semantics remain active: Operation identity, Workspace
lease/epoch/expiry, optimistic revision/CAS, and SQLite transactions protect
logical callers. General independent-process Workspace publication on Windows
remains `PARTIAL`; it is outside the normal external-Runtime ownership path.

Pong Core remains frozen as an offline-first, local-first,
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
passes. At that checkpoint, MCP, HTTP, SDK adapters, remote execution, and
production authentication were `NOT_PROVEN`; M4-013 now supersedes only the
HTTP transport item.

**HTTP contract materials:** [`M4_HTTP_REMOTE_TRANSPORT.md`](../architecture/M4_HTTP_REMOTE_TRANSPORT.md),
[`ADR-M4-013-http-remote-transport.md`](../decisions/ADR-M4-013-http-remote-transport.md),
[`ADR-M4-014-http-production-hardening.md`](../decisions/ADR-M4-014-http-production-hardening.md),
[`M4_REAL_REMOTE_E2E.md`](../architecture/M4_REAL_REMOTE_E2E.md),
[`ADR-M4-015-real-remote-e2e.md`](../decisions/ADR-M4-015-real-remote-e2e.md),
[`http_remote_transport.rs`](../../tests/http_remote_transport.rs),
[`http_hardening.rs`](../../tests/http_hardening.rs), and
[`real_remote_e2e.rs`](../../tests/real_remote_e2e.rs).

**Next Action:** Decide the intended read-sharing/project membership policy for
registered Agents, then gate OBSERVE queries and diff without breaking the
frozen Protocol v1.0 wire shape. SDK and MCP remain deferred. Production TLS
termination, secret-manager
integration, Windows credential-file ACL validation, global connection/parser
queue limits, slow-client deadlines, production observability export, Internet
deployment, and Linux/macOS parity remain `NOT_PROVEN`. Rollback remains a
Protocol v1.0 capability gap. WebSocket is deferred and MCP remains an optional
adapter; neither enters Pong Core.


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
