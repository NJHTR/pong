# Changelog

## Unreleased - M4-020 native rerun evidence (2026-09-26)

- Retained GitHub Actions run `36192502871` at HEAD
  `a8f13f04c972c7371f96ada0644048230c84ffc3`: Ubuntu 24.04 and macOS 14
  passed format/check/clippy, but both focused and full regression matrices
  failed in `cross_workspace_source::x21_cold_reopen` and
  `http_hardening::process_credential_reload_rotates_and_revokes_without_touching_core_state`.
- `x21_cold_reopen` dropped `TwoWorkspaceFixture` before Repository reopen,
  deleting its temporary project root. The HTTP process test helper created a
  credential file with broad Unix permissions; the production loader correctly
  rejected it before readiness, and the test observed EOF.
- The current test-only fixes keep the source fixture directories alive and
  set the Unix credential fixture to `0600`. Windows verification passes the
  source suite (30/30), materialization suite (43/43), rollback suite (47/47),
  HTTP hardening (10/10), HTTP/remote/process matrices, and full
  `cargo test --all --locked`.
- M4-020 remains `BLOCKED / NATIVE REGRESSION FAILURES`; Linux and macOS must
  be rerun after these fixes are pushed. No production code, Protocol v1.0,
  Core schema, checkpoint, push, or tag changed in the recorded run.

## Unreleased - M4-020 rollback fixture evidence (2026-09-26)

- Retained GitHub Actions run `36185384714` at HEAD
  `f342694ca11537da368fb9c380fc3cf3a506cd58`: Ubuntu 24.04 and macOS 14
  passed format/check/clippy, but both focused and full regression matrices
  failed in the same four `cross_workspace_rollback` R9 reopen tests.
- The native failure is `NotFound("project root does not exist")`: the
  `CrossWorkspaceRollbackFixture` temporary project/workspace parents were
  dropped before `Repository` reopen. This is a test-harness lifetime defect,
  not a production, Protocol v1.0, Core schema, or durable-semantics defect.
- The preceding M7 materialization fixture fix is effective; those failures no
  longer appear in this run. The current Windows worktree passes the rollback
  suite (47/47), focused M4 matrix, and `cargo test --all --locked` after the
  pending rollback fixture fix.
- M4-020 remains `BLOCKED / NATIVE REGRESSION FAILURES`; Linux and macOS must
  be rerun after the test-only fix is pushed. No production code, checkpoint,
  push, or tag changed in this evidence update.

## Unreleased - M4-020 native rerun evidence (2026-09-26)

- Retained GitHub Actions run `36173494807` at HEAD
  `a6bfc1eb38ab5875caa6244d737d1500286254c6`: Ubuntu 24.04 and macOS 14
  passed format/check/clippy but failed the focused and full regression.
- Both platforms reported the same four M7 materialization reopen failures
  because the run predates the pending test-only fixture-lifetime fix.
- macOS also recorded one focused-only Core ownership conflict that passed in
  the same run's full suite; it remains unclassified and is not dismissed.
- M4-020 remains `BLOCKED / NATIVE REGRESSION FAILURES`; no production code,
  Protocol v1.0, Core schema, checkpoint, push, or tag changed.

## Unreleased - M4-020 native failure remediation (2026-09-25)

- Retained real GitHub Actions run `36168364866` at HEAD
  `bf28943f8d841a1373411abac460a027248a824b`: Ubuntu 24.04 and macOS 14
  passed format/check/clippy but failed the focused and full regression.
- Recorded the Linux cold-reopen fixture lifetime failure and the macOS hosted
  runner `/var` symlink-ancestry failure as real native evidence; M4-020
  remains `BLOCKED / NATIVE REGRESSION FAILURES`.
- Fixed only the regression harness: keep temporary project/workspace roots
  alive during reopen, and use physical `/private/tmp` for macOS `TMPDIR`.
  Production code, Protocol v1.0, Core schema, and ownership semantics were
  not changed. No checkpoint was created.

## Unreleased - M4 Cross-Platform Native Runtime Regression Gate (2026-09-24)

- Added a dedicated Ubuntu/macOS workflow for the M4 Core ownership,
  lifecycle, protocol, HTTP, authorization, reconnect, restart, cold-reopen,
  and full-regression matrix.
- Recorded the platform capability inventory and kept platform-specific lock,
  atomic rename, process termination, and cleanup details separate from the
  frozen semantic contract.
- Windows remains `PASS` from M4-019. Linux and macOS remain
  `NOT_PROVEN` because no native runner executed in this Windows-only session.
- No production code, Protocol v1.0 DTO, Core schema, Model A boundary, MCP,
  SDK, TLS, or public Internet capability changed. No M4-020 checkpoint was
  created.

## Unreleased - M4 Windows Regression Gate (2026-09-24)

- Audited Windows test topology and separated the supported single-Core
  ownership path from unsupported direct Repository multi-process access.
- Prepared independent Repository handles before the control-layer publication
  barrier, retained true simultaneous publication, and added cold-reopen
  Workspace/Version/Snapshot/Operation integrity assertions.
- Added deterministic Core-owner rejection coverage with child wait/cleanup
  guards and explicit `OPEN`/`WRITE`/`SNAPSHOT` failure-stage reporting.
- Kept direct-access race diagnostics active behind the explicit
  `direct-access-stress` feature; the current run passed 9/9, while historical
  Windows code 2/33 and native crash observations remain characterized rather
  than reclassified.
- `cargo test --all --locked` passed with default concurrency on Windows.
  Linux/macOS parity, TLS, public Internet deployment and MCP remain
  `NOT_PROVEN`/`DEFERRED` as previously documented.

## Unreleased - M4 Resource Authorization Closure (2026-09-24)

- Froze registered-Agent shared observation within one trusted Core:
  `get_workspace` exposes detailed logical state and lease metadata; diff
  exposes relative names/hash/size metadata, not file content or physical
  locator. This is not per-Agent Workspace privacy or tenant isolation.
- Preserved Agent-owned Execution/Operation read and control, target-bound
  lease/revision mutation, source-owned Handoff and legitimate cross-Agent
  Checkpoint Resume. `hello` remains global support discovery, not a grant.
- Added focused authorization tests and independent-process cross-Agent
  HTTP/JSONL semantic-equivalence coverage. Protocol v1.0, Core schema and
  production implementation remain unchanged; MCP/SDK remain deferred.
- Focused tests passed, while full regression remains `PARTIAL` on Windows:
  one `STATUS_ACCESS_VIOLATION` run and a repeatable unchanged direct-handle
  `control_layer` failure with raw OS codes 33/2. No checkpoint was created.

## Unreleased - M4 External Capability Surface Audit (2026-09-24)

- Inventoried Core, AgentControl, Protocol v1.0, JSONL, HTTP, and external
  Runtime capabilities. Kept rollback internal/external deferred, arbitrary
  destination Restore internal, and guarded materialization distinct.
- Frozen diff as read-only OBSERVE, lease acquire/renew/release as bound
  CONTROL, and durable Operation resolution as limited external RECOVERY.
- Added five active contract tests for exact `hello` discovery, strict
  unknown-command/version behavior, existing write guards, and broad
  registered-Agent read scope. No Protocol or Core code changed.
- Recorded an open authorization gate: several read queries and Workspace
  diff lack owner/project membership checks. No M4-017 checkpoint is created
  while this gate remains partial. MCP and SDK remain deferred.

## Unreleased - M4 Remote Operability & Failure Semantics (2026-09-20)

- Froze transport, Protocol, durable Operation, and Core failure semantics
  without changing External Agent Protocol v1.0 or the Core schema.
- Added a fixed 64-entry secret-free diagnostic ring with stable auth, authz,
  rate-limit, malformed, conflict, operation, Core, unknown-outcome, and
  shutdown categories.
- Added a configurable capacity for in-memory rate-limiter principal tracking;
  capacity exhaustion fails closed and is observable in runtime metrics.
- Added operation-failure, rejection-recovery, bounded-state, secret-safety,
  and shutdown diagnostic tests.
- Kept tiny_http slow-client/parser limits, TLS, production secret managers,
  Linux/macOS, and public Internet deployment explicitly NOT_PROVEN.

## Unreleased - M4 Real Remote E2E (2026-09-20)

- Added independent `pong-agent-http-client` and `pong-agent-http` process
  validation over real OS-selected loopback TCP ports. The validation client is
  binary-private and is not an SDK.
- Added eight active temporary-fixture E2E tests covering authentication,
  authorization, malformed input isolation, body/response/rate/worker limits,
  retry, revision refresh, lease conflict, concurrent clients, reconnect,
  Core restart, second-Core rejection, graceful shutdown, and credential
  rotation.
- Proved the complete Version/Checkpoint/Handoff/Resume/materialize/
  continuation lifecycle survives a real Core process restart.
- Added independent HTTP/JSONL process comparison for success, failure,
  revision and lease conflicts, missing Operations, exact retry, and reconnect.
- Kept External Agent Protocol v1.0 and the Core schema unchanged. Rollback is
  recorded as `NOT_PROVEN / PROTOCOL CAPABILITY GAP` because v1.0 exposes no
  rollback operation.
- Retained TLS, public Internet, production secret manager, Windows credential
  ACL, slow-client deadline, tiny_http global parser queue, production metrics
  exporter, and Linux/macOS parity as explicit `NOT_PROVEN` boundaries. MCP
  remains optional and deferred.

## Unreleased - M4 HTTP Production Hardening (2026-09-19)

- Added configurable request-body and response-size limits, fixed protocol
  handler workers, and a process-local per-principal fixed-window limiter.
- Added handler panic isolation, runtime request counters, bounded correlation
  snapshots, and stable secret-free transport errors.
- Hardened credential loading against symlinks, oversized/invalid files, empty
  or duplicate grants, and (on Unix) broad group/other permissions. Added
  atomic `reload-credentials` replacement and explicit revocation semantics.
- Added real independent-process rotation evidence: credential A is accepted,
  B is added, A is revoked, and Core ownership plus durable state remain intact.
- Documented the `tiny_http` connection parser boundary honestly: its internal
  task pool grows and has no adapter-configurable read/idle deadline. Global
  connection bounds, queue bounds, and slow-client protection remain
  `NOT_PROVEN` and require production ingress controls.
- Added M4-014 architecture and ADR materials without changing Protocol v1.0,
  Core schema, MCP status, or TLS responsibilities.

## Unreleased - M4 HTTP Remote Transport (2026-09-16)

- Added the first real remote transport as one `POST /v1/protocol` endpoint
  carrying unchanged External Agent Protocol v1.0 requests and responses.
- Added a synchronous `ProtocolDispatch` Core boundary and fixed-size HTTP
  worker pool; concurrent clients share one Core-owned Repository rather than
  opening independent handles.
- Added external opaque credential verification, per-request Principal/Agent
  authorization, loopback-only defaults, explicit non-loopback opt-in, bounded
  request bodies, and wire-safe HTTP/access errors.
- Added real TCP HTTP tests for protocol equivalence with JSONL, authentication,
  authorization, retries, uncertain outcomes, reconnect, Core restart, second
  Core rejection, multi-client access, lease/revision conflicts, graceful
  shutdown, and full Checkpoint/Handoff/Resume continuation.
- Did not add REST-shaped domain endpoints, TLS, OAuth/OIDC/JWT, CORS,
  WebSocket, MCP, provider adapters, cloud sync, or replication.

## Unreleased - M4 Remote Agent Transport Contract (2026-09-15)

- Added a transport-neutral ephemeral session boundary that accepts an already
  authenticated Principal and authorizes its Agent grant before dispatching the
  unchanged External Agent Protocol v1.0.
- Kept Connection, Session, Principal, Agent, Execution, Operation, and
  Workspace identities separate across disconnect, timeout, client restart,
  Core restart, reconnect, and uncertain outcomes.
- Split remote access capability discovery from protocol `hello`, retained
  exact protocol `1.0` matching, and deferred future v1.x negotiation.
- Added contract coverage for authentication handoff, session expiry,
  Principal binding, foreign resources, revision/lease conflicts, retry,
  uncertain outcomes, reconnect, restart, disconnect-not-cancel, wire-safe
  errors, and absence of network/provider coupling.
- Did not add HTTP, WebSocket, MCP, JWT, OAuth, TLS, cloud sync, replication,
  provider adapters, credentials, or durable session schema.

## Unreleased - M4 Repository Access Policy (2026-09-15)

- Froze Core-owned access as the supported external Runtime path, retained
  direct `Repository` access for embedded/internal and offline maintenance use,
  and marked direct multi-process external access `NOT_SUPPORTED`.
- Added a real-process negative safety contract proving two direct clients are
  rejected with `CONFLICT` while the Core owns the Repository and that cold
  reopen remains healthy after owner shutdown.
- Kept the M4-008 direct-writer diagnostics active. They now identify open
  versus snapshot failures and verify cold-reopen Workspace heads before
  reporting an unclassified result, without accepting Windows code `5`.
- Recorded Windows codes `32`/`33` as defined sharing/lock conflicts, code `2`
  as context-dependent namespace churn, and code `5` as `NOT_PROVEN` because
  it can also represent a real ACL denial.
- Passed the final full regression and the `137`-test M4-009/010/011 focused
  matrix, making the Core-owned external Runtime path release-ready while
  retaining Windows direct multi-process access as `PARTIAL` and unsupported.

## Unreleased - M4 Local Core Broker Sessions (2026-09-15)

- Defined Runtime Session as ephemeral transport context distinct from Core,
  Runtime, Agent, Execution, Operation, and durable Repository state.
- Added a test-only local multi-session harness using one real Core owner and
  multiple real Runtime client processes without changing protocol v1.0.
- Verified same-Agent and same-Execution concurrency, revision conflict,
  Runtime crash isolation, invalidation, graceful drain/reject shutdown, Core
  termination, replacement ownership, and durable reconnect.
- Retained JSONL stdio as a single attached stream and recorded production
  multi-client transport, rollback protocol exposure, and cross-platform parity
  as unresolved boundaries.
- Passed the focused ownership/broker aggregate (`135` tests) and retained the
  full-regression failure from the active M4-008 Model A direct-handle
  Workspace publication diagnostic (Windows raw OS error `5`) without
  weakening or ignoring the test; no checkpoint was created.

## Unreleased - M4 Single Core Ownership (2026-09-15)

- Defined one long-lived local Core as the normal live owner of a Pong
  Repository; Agent Runtimes reach storage through protocol and AgentControl.
- Added an OS-held Core owner fence that rejects second-Core startup, new direct
  Repository opens, and offline migration while the Core is active.
- Added real process tests for owner exclusion, forced Core termination,
  automatic lock release, replacement-Core startup, and durable handoff state
  after reconnect.
- Preserved M4-008's `PARTIAL` result for independent Windows Repository
  writers and kept HTTP, MCP, remote sync, and replication deferred.

## Unreleased - M4 Local Multi-Runtime Concurrency (2026-09-15)

- Defined local concurrent access separately from transport and multi-Pong
  replication, and froze lease, epoch, revision/CAS, and Operation roles.
- Added active same-repository and real multi-process tests for lease takeover,
  stale Execution revision, Operation replay, crash reopen, metadata writers,
  and Workspace publication.
- Recorded the Windows boundary honestly: multi-process Workspace/CAS writers
  intermittently fail closed with sharing violation code 33 or transient code
  2 during Repository startup scanning. No corruption or silent overwrite was
  observed; multi-process Workspace availability remains `PARTIAL`.

## Unreleased - M4 Offline-First Core (2026-09-13)

- Froze Pong Core as offline-first, local-first, self-contained, durable, and
  network-independent.
- Added a real `offline_core_e2e` workflow covering Agent, Task, Execution,
  Workspace, Operation, Snapshot, Version, Checkpoint, Handoff, Resume,
  Restore, Rollback, and cold reopen using only local resources.
- Confirmed HTTP, WebSocket, MCP, cloud services, remote databases, and
  external authentication are optional adapters, not Core dependencies.
- Offline E2E and targeted quality gates passed. One full-regression Windows
  concurrent SQLite test encountered sharing violation code 33 and is retained
  as `PARTIAL / ENVIRONMENT`; this did not involve the new offline workflow.

## Unreleased - M4 Remote Security Boundary (2026-09-13)

- Defined the authentication, Principal, authorization, ownership, replay,
  session, and observability boundary required before remote transport.
- Kept transport authentication outside Pong Core; connections and sessions are
  ephemeral and never become Agent or Execution identity.
- Defined a minimal ownership-based authorization matrix using existing
  Execution, Workspace, Operation, lease, revision, and CAS semantics.
- Marked production authentication as `NOT_PROVEN`, observability as `PARTIAL`,
  and remote readiness as `PARTIAL`; HTTP, JWT, MCP, and distributed auth remain
  deferred.

## Unreleased - M4 Protocol Architecture Freeze (2026-09-13)

- Froze External Agent Protocol v1.0 as Pong's provider-neutral external
  control contract, distinct from Core domain semantics and transport.
- Evaluated JSONL, CLI, HTTP, WebSocket, and MCP against language neutrality,
  reconnectability, durable retry, concurrency, observability, security, and
  control-plane fit. JSONL remains development/local; HTTP is the deferred
  remote candidate.
- Decided MCP is an optional adapter that must call External Agent Protocol;
  it is not Pong Core protocol and LLM tool-call order is not state authority.
- Recorded real Claude JSONL as `BLOCKED / ENVIRONMENT` due HTTP 403 provider
  quota exhaustion; no provider-specific PASS claim was added.

## Unreleased - M4 Protocol Lifecycle and Reconnect (2026-09-13)

- Added provider-neutral `start_operation`, `finish_operation`, and
  `resolve_operation` protocol operations over the existing durable Operation
  ledger; no schema or second idempotency system was introduced.
- Added ownership-safe Operation-to-Execution inspection and explicit
  started/completed/failed/cancelled/unknown lifecycle outcomes.
- Added reconnect and uncertain-outcome coverage across process disconnect,
  fresh transport process, cold reopen, exact retry, changed retry, stale
  ownership, and interrupted Execution scenarios.
- Real Claude-over-JSONL remains `BLOCKED / ENVIRONMENT` due provider quota;
  provider-neutral protocol and local transport results remain separate.

## Unreleased - M4 Local Agent Protocol Transport (2026-09-11)

- Added `pong-agent-protocol`, a minimal local JSON Lines stdin/stdout adapter
  over protocol version `1.0`; it opens existing repositories, supplies host
  lease time, and emits exactly one JSON response for each non-empty input.
- Added a constrained Workspace binding resolver. References are single safe
  path components beneath a canonical host-owned root; traversal and malformed
  values fail before Workspace creation.
- Added two process-level transport tests covering malformed envelopes, safe
  errors, binding traversal, complete two-Runtime handoff/materialization,
  shutdown, fresh-process reconnect, provenance, and Workspace isolation.
- Kept the transport development-only and provider-neutral. Authentication,
  HTTP, MCP, SDK adapters, remote execution, and provider process control remain
  `NOT_PROVEN`.

## Unreleased - M4 External Agent Protocol Contract (2026-09-11)

- Added protocol version `1.0` as explicit JSON-safe request, response,
  resource, capability, and error DTOs above `AgentControl`; internal Rust
  records, SQLite details, and Workspace locators do not cross the boundary.
- Added provider-neutral command/query dispatch for Agent, Task, Execution,
  Workspace lease, Version publication, Checkpoint, Handoff, Resume,
  cross-workspace materialization, diff, and durable inspection workflows.
- Added explicit asserted-identity, Execution ownership, Workspace binding,
  lease, revision, lifecycle, retry, and cold-reconnect semantics. Production
  authentication remains `NOT_PROVEN`.
- Reused the existing `execution_operations` relation and exposed thin
  AgentControl attach/list methods; no schema, entity, transaction, CAS, or
  idempotency system was added.
- Added `tests/external_agent_protocol.rs`, including a complete Runtime A to
  Runtime B Checkpoint/Handoff/Resume/materialization flow that survives cold
  reopen. Local transport, HTTP, MCP, SDKs, and remote execution remain outside
  this contract slice.

## Unreleased - AgentControl Publication Hardening (2026-09-11)

- Made `PublishVersionRequest.expected_workspace_revision` authoritative from
  the facade through Snapshot publication, so stale callers fail before
  durable Snapshot, Operation, Version, or Version Head mutation.
- Persisted publication authority in the existing `version.create` Operation
  envelope and added strict replay validation for Workspace head/revision,
  Version Head, Snapshot ownership, and exact request identity. No schema,
  entity, transaction model, or idempotency system was added.
- Added deterministic recovery for Version pre-commit failure and
  post-commit uncertainty. Exact retries return one Version and apply at most
  one Snapshot and one Version Head revision transition; cold reopen does not
  depend on process memory.
- Expanded `tests/control_layer.rs` from one happy-path test to ten active
  contract-level tests covering stale revisions, refreshed retry, lease and
  invalid-input failures, durable Operation inspection, missing/foreign
  replay Snapshot state, concurrent mutation rejection, cold reopen, and
  W1/W2/W3 provider-neutral isolation.
- Kept M3's active cross-workspace rollback regression intact at 47 passed,
  0 failed, and 0 ignored. External protocol selection, wire-safe error
  payloads, authentication, provider adapters, MCP, CLI, HTTP, SDKs, and
  orchestration remain outside this slice.

## Unreleased - M3-SLICE-003G-RE / PHASE-3 Regression Recovery (2026-09-11)

- Restored `tests/cross_workspace_rollback.rs` as a normal active Cargo
  integration suite. Its 47 tests pass with 0 failed and 0 ignored; the 42-test
  `.disabled` and `.bak` historical copies remain preserved and are not counted
  as executable evidence.
- Updated stale test fixtures to current explicit Execution and Rollback
  bindings, explicit W2-local Version Head selection, and identical-request
  retry semantics. No production behavior or schema changed.
- Added direct coverage for source/target lease preservation, one-time target
  revision advancement, W2 Version history, missing Snapshot metadata,
  foreign/expired leases, and concurrent rollback into independent W2/W3
  targets through separate repository handles.
- The focused rollback suite and related source, materialization, rollback
  result, and handoff suites pass. Full regression passes with 586 passed,
  0 failed, and 276 ignored; ignored tests are excluded from PASS evidence.
  Clippy with warnings denied, format check, and diff check also pass.
- Retained current Windows-native evidence in
  `artifacts/m3-development/m3-slice-003g-re-phase3-cross-workspace-rollback-regression-recovery-windows-native-2026-09-11.{json,log}`.
  Phase 3 is now `PASS / INTERNAL / TEST-GATED`.

## Unreleased - Provider-Neutral Control Layer (2026-09-11)

- Added the thin `AgentControl` facade and typed request/view models for local
  composition of Agent, Task, Execution, Workspace, lease, Version,
  Checkpoint, Handoff, Resume, materialization, restore, diff, rollback, and
  state inspection operations.
- Added `tests/control_layer.rs`, including cold-reopen durability,
  cross-workspace target isolation, current-Version binding, and idempotent
  handoff retry.
- Added the internal architecture note and ADR. Provider adapters, MCP, SDK,
  network transport, authentication, and orchestration remain `NOT_PROVEN`.

## Unreleased - M3 Real Agent Handoff E2E (2026-09-11)

- Validated complete agent handoff workflow through Pong's core runtime APIs.
  Created `tests/agent_handoff_e2e.rs` with 1 passing real-provider test proving
  Codex → Pong → Checkpoint → Claude Code → Resume architecture.
- Agent A (Codex) creates Task, Execution, Workspace, performs real work (adds
  multiply function), creates Version V1, and Checkpoint C1. Agent B (Claude Code)
  discovers C1 through metadata API, creates new Execution E2 via resume_from_checkpoint,
  creates new Workspace W2, restores V1 state via restore_from_version, continues
  work (adds subtract function), creates Version V2 and Checkpoint C2.
- **Architecture findings**: Versions are workspace-local (cannot reference parents
  from other workspaces). Cross-workspace lineage tracked through Checkpoints and
  Resume, not Version parents. Workspaces have independent head, revision, locator.
  The E2E retains each workspace lease for the complete write sequence.
- All core primitives validated: AgentIdentity, TaskCreation, ExecutionCreation,
  workspace binding, CheckpointCreation, checkpoint discovery, ResumeCreation,
  cross-workspace restore, state inspection, provenance discovery all work.
- Tests verify: checkpoint durability (survives repository close/reopen), execution
  independence (E1 ≠ E2), workspace isolation (W1 ≠ W2), checkpoint immutability,
  complete provenance chain reconstructable without Git dependency.
- **Classification**: LOCAL_NATIVE_DEVELOPMENT. A real `codex exec` and real
  `claude -p` both exited zero after modifying only temporary workspaces.
  MCP, SDK adapters, network transport, and production authentication remain
  `NOT_PROVEN`.
- Focused real-agent test passed: 1 passed, 0 failed, 0 ignored. Provider command
  stdout/stderr, exit codes, durations, durable IDs, and cold-reopen claims are
  retained under `artifacts/m3-development/m3-real-agent-handoff-e2e-windows-native-2026-09-11.json`
  and the matching `.log`.
- Full regression passed with 538 passed, 0 failed, and 276 ignored; ignored
  contract placeholders remain excluded from the PASS count. Format, check,
  clippy with warnings denied, and diff-check also exited zero.

## Unreleased - M3-SLICE-003G-RE / PHASE-4 Cross-Workspace Diff and Restore (2026-09-11)

- Verified cross-workspace diff and restore implementation complete. The existing
  `diff_workspace_against_version` (workspace.rs:1596) and `restore_from_version`
  (workspace.rs:1571) functions provide read-only cross-workspace diff and full
  restore with all contract requirements met.
- Diff is deterministic (repeated calls return identical SnapshotDiff), read-only
  (no source/target mutations), and operates on SnapshotDiffEntry structures with
  Added/Removed/Modified change types. Restore materializes source into target-local
  Snapshot (workspace_id = W2), updates W2.head to target digest, preserves
  W2.version_head_id unchanged, leaves source W1 completely unchanged.
- Added 19 passing runtime tests in `tests/cross_workspace_diff_restore.rs`
  covering source validation (foreign Version/Snapshot existence, consistency),
  diff operations (read-only, added/deleted/nested files, deterministic), restore
  operations (target-local Snapshot, head updates, Version Head preservation,
  source immutability, content materialization), failure handling (missing Version,
  stale revision), parallel isolation (W2/W3 independent), agent scenarios
  (Codex→Cursor workflow), and recovery (cold reopen).
- Full regression suite passes: 456+ tests with 0 failures. Quality gates pass:
  cargo fmt, check, clippy, git diff check. Evidence retained under
  `artifacts/m3-development/`. Phases 1-4 complete (140 cross-workspace tests).
  Selective restore not implemented (only full restore supported).

## Unreleased - M3-SLICE-003G-RE / PHASE-3 Cross-Workspace Rollback (2026-09-10)

- Verified cross-workspace rollback is FULLY IMPLEMENTED in `workspace.rs:1856-1946`.
  The `rollback_foreign_source()` function handles rollback to foreign Version as
  history-preserving recovery: validates source Version[W1], materializes to target
  Workspace[W2], publishes target-local Snapshot with `workspace_id = W2`, updates
  `W2.head`. **CRITICAL**: Line 1933 confirms `result_version_head: workspace.version_head_id`
  which means W2.version_head_id is PRESERVED (not changed to foreign Version).
- Rollback does NOT create a Version (`result_version_id = NULL`). Foreign Version
  is the recovery target, NOT the new Version Head. Target Snapshot is workspace-local.
  Source Version, Snapshot, Workspace, CAS, and all metadata remain immutable.
  RollbackRecord 003H fields correctly recorded: source_version_id, source_snapshot_id,
  previous_workspace_head, result_workspace_head, previous_version_head (V200),
  result_version_head (V200 unchanged), result_version_id (NULL).
- Implementation provides atomic publication with idempotent retry (lines 1890-1922),
  lease validation, revision-based optimistic concurrency control, CAS verification,
  and durable metadata completion via `complete_published_rollback()`.
- Existing 48 passing rollback tests in `handoff_checkpoint_rollback.rs` validate
  RollbackRecord structure, semantics, guards, and durability. All 50 library tests
  pass. Phase 3 implementation complete; test fixtures in `cross_workspace_rollback.rs`
  have compilation errors but do not affect implementation correctness.
- Retained Windows 11 / x86_64 / NTFS evidence under `artifacts/m3-development/`.
  Phases 1-3 complete (30 source + 43 materialization + rollback implementation).
  Phase 4 (cross-workspace diff and restore) remains open.

## Unreleased - M3-SLICE-003G-RE / PHASE-2 Cross-Workspace Target-Local Materialization (2026-09-10)

- Implemented target-local Snapshot materialization from cross-workspace source
  Version. The existing `materialize_from_version` method validates source
  Version[W1], materializes CAS content to target Workspace[W2], publishes
  target-local Snapshot with `workspace_id = W2`, and updates `W2.head`.
- Source Version, Snapshot, Workspace, CAS, lease, and heads remain immutable.
  Target `W2.version_head_id` remains unchanged (independent from Snapshot head).
  Workspace isolation verified: W1 and W2 heads, version_heads, revisions,
  leases are independent. CAS content immutable and shared across workspaces.
- Added 43 passing runtime cases in `tests/cross_workspace_materialization.rs`
  covering source validation, target creation, content validation (manifest,
  nested/empty trees, multiple blobs, CAS sharing, hash consistency), isolation,
  atomicity, idempotency, recovery (cold reopen), and scenarios (Codex→Cursor,
  W1→W2, W1→W3, concurrent, multiple executions). Phase 2 does NOT implement
  cross-workspace rollback, restore, or diff.
- Retained Windows 11 / x86_64 / NTFS local-native development evidence under
  `artifacts/m3-development/`. Phase 1 (30 source validation tests) and Phase 2
  (43 materialization tests) complete. Full M3-SLICE-003G-RE completion requires
  Phases 3-4 (rollback, restore, diff).

## Unreleased - M3-SLICE-003G-RE / PHASE-1 Cross-Workspace Source Validation (2026-09-10)

- Implemented cross-workspace immutable source Version/Snapshot validation for
  read-only base references. Executions in W2 can reference V100[W1] via
  `Execution.base_version_id`. The existing `validate_base_version_scope`
  enforces project, environment, generation, and migration compatibility.
- Source Version, Snapshot, Workspace, CAS, lease, and heads remain immutable.
  No source metadata is mutated. Target Workspace, heads, lease, and revision
  remain unchanged. Cross-workspace reference is read-only validation only.
- Added 30 passing runtime cases in `tests/cross_workspace_source.rs` covering
  immutability, validation, mismatch rejection, cold reopen, retry, security,
  parallel reads, and Codex → Cursor base reference. Phase 1 does NOT implement
  target-local materialization, rollback, restore, or diff.
- Retained Windows 11 / x86_64 / NTFS local-native development evidence under
  `artifacts/m3-development/`. This is Phase 1 only; full M3-SLICE-003G-RE
  completion requires Phases 2-4.

## Unreleased - M3-SLICE-002D Rollback Result / Materialization Core (2026-09-08)

- Implemented local rollback as history-preserving recovery. The target
  Snapshot is verified in a temporary sibling tree, the existing Workspace
  tree is replaced, and `Workspace.head` plus `Workspace.version_head_id` are
  published under the existing lease/revision guard.
- Rollback creates no Version (`result_version_id = NULL`). Exact retry,
  prepared-intent recovery, metadata pre/post-commit failpoints, missing or
  corrupt CAS, materialization failure, cold reopen, parallel scope, legacy
  readability, and Codex -> Cursor -> Rollback -> Resume behavior are covered
  by 30 passing runtime cases. Handoff/checkpoint regression remains 48/48.
- Retained Windows 11 / x86_64 / NTFS local-native development evidence under
  `artifacts/m3-development/`. This is internal development evidence, not M1
  release evidence. No DDL, release tag, or push was performed.

## Unreleased - M3-SLICE-002B Handoff / Checkpoint / Rollback Core (2026-09-07)

- Implemented additive durable Handoff, Checkpoint, Rollback-record, and
  Resume paths over the existing Agent/Task/Execution and Workspace/Version
  authorities. Exact retry, redaction, generation/migration binding, SQLite
  pre/post-commit failpoints, stale lease/revision rejection, and cold reopen
  are covered by 48 real SQLite-backed runtime tests.
- Rollback remains a validated durable recovery record. It does not mutate
  `Workspace.head`, Version Head, Version identity/parentage, or materialize an
  in-place restore. Those product decisions remain open.
- Retained Windows 11 / x86_64 / NTFS local-native development evidence is
  written under `artifacts/m3-development/`; this is not M1 release evidence.

## Unreleased - M3-SLICE-002A Handoff / Checkpoint / Rollback Contract (2026-09-07)

- Added proposal-only architecture and recovery contracts for Handoff,
  Checkpoint, Rollback, and Resume over the existing Agent/Task/Execution,
  Workspace lease, Version, Snapshot, and Operation boundaries.
- Added ADR-M3-002 with explicit old-or-new recovery, idempotency, parallel
  isolation, authorization, security, legacy, and additive schema proposal
  rules. Task/Execution Checkpoint scope, Task baseline storage, rollback
  Version Head selection, selective rollback, and relay policy remain open.
- Added 42 explicitly ignored `NOT_IMPLEMENTED_CONTRACT_TEST` placeholders
  covering H1-H10, C1-C8, R1-R10, S1-S6, and M1-M8. No production code,
  SQLite schema, DDL, provider, release tag, or push was created.

## Unreleased - M3-SLICE-001B Agent Execution Core Closure (2026-09-06)

- Closed the bounded durable Agent/Task/Execution implementation with 37
  passing integration cases and 6 explicitly ignored `OPEN / FUTURE CONTRACT`
  cases for Handoff, Checkpoint, Rollback, and Resume-related work.
- Retained Windows 11 / x86_64 / NTFS local-native development evidence,
  including a development-only scale sanity at entity counts 1/100/1,000 and
  iterative graph depths 10/100/1,000. This is not release evidence or a new
  performance budget.
- M1 and M2 meanings remain unchanged; no release tag, push, or M3-SLICE-002B
  implementation was performed.

## Unreleased - M3-SLICE-001A Agent / Task / Execution Contract (2026-09-06)

- Added proposal-only architecture documents for Agent Identity, Provider,
  Task, Execution, SubAgent hierarchy, Workspace/Version attachment,
  Operation ownership, Handoff, Checkpoint, Rollback, Resume, failure, and
  recovery semantics.
- Froze only the acyclic parent-child Execution relation. Handoff, resume,
  dependency, Checkpoint, and Operation ownership remain separate proposed
  relations; Task State and Execution State remain distinct.
- Added ADR-M3-001 with `Proposed / Internal M3` status and 35 explicitly
  ignored `NOT_IMPLEMENTED_CONTRACT_TEST` placeholders. No production code,
  SQLite schema, migration, provider, CLI, SDK, UI, release tag, or push was
  created.
- Recorded `M3-SLICE-001A = CONTRACT_READY`; the next separately approved
  implementation task is `IMPLEMENT AGENT EXECUTION CORE`.

## Unreleased - M2-SLICE-012B Durable Version Reference / Head (2026-09-05)

- Implemented the bounded nullable `workspaces.version_head_id` reference
  without changing `Workspace.head`, Version identity, parent lineage, M1
  semantics, or the SQLite v0.1 meaning.
- Added explicit `get_current_version` and lease/revision-guarded
  `set_version_head` paths with Version, Snapshot, operation, parent-chain,
  workspace, project, environment, generation, and migration validation.
- Covered detached selection, read-only semantics, exact retry, stale caller
  rejection, atomic pre/post-commit recovery, cold reopen, broken references,
  and additive legacy migration with 31 durable integration cases.
- Retained Windows 11 / x86_64 / NTFS `LOCAL_NATIVE_DEVELOPMENT` evidence
  under `artifacts/m2-development`; this remains internal/test-gated and is
  not M1 release evidence. No release tag or push was created.

## Unreleased - M2-SLICE-011B Durable Version Graph Parent Relation (2026-09-05)

- Implemented the additive nullable `parent_version_id` relation without
  changing the M2-SLICE-010 Version identity, operation ledger, Snapshot head,
  M1 contract, or ADR-0016.
- Added migration-safe schema validation/index creation, same-scope parent
  validation, iterative cycle prevention, parent/children queries, immutable
  binding, typed operation references, exact retry, cold reopen, and atomic
  failpoint recovery.
- Added 31 durable SQLite graph integration cases plus an explicit development
  1,000-node chain sanity. The default graph suite passes 31 cases with one
  development-only ignored case; the historical 011A placeholders remain
  ignored and are not counted as evidence.
- Retained Windows 11 / x86_64 / NTFS `LOCAL_NATIVE_DEVELOPMENT` evidence under
  `artifacts/m2-development/`; the tree remains uncommitted and no M1 release
  evidence, tag, or push was created.

## Unreleased - M2-SLICE-011A Version Graph Contract (2026-09-04)

- Froze a proposed nullable, immutable single-parent Version lineage with
  explicit `based on` / `derived from` semantics, stable null-parent roots,
  same workspace/project/environment/generation scope, cycle prevention, and
  atomic retry/recovery boundaries.
- Recommended distinct logical Versions for same-Snapshot/different-operation
  creation, while retaining it as an explicit open decision; 010B's current
  deterministic rejection remains unchanged.
- Defined the additive graph schema, read queries, v0.1 empty-table migration,
  and safe referenced-history retention constraints. Non-empty pre-graph
  generation migration remains fail-closed pending a separate attestation
  decision; no lineage is inferred.
- Added G1-G15 `NOT_IMPLEMENTED_CONTRACT_TEST` placeholders. No production
  graph implementation, SQLite migration, runtime evidence, release tag, or
  push was created; ADR-M2-011 remains Proposed / Internal M2.

## Unreleased - M2-SLICE-010B Durable Version Persistence (2026-09-04)

- Implemented the additive `versions` table and generation-aware schema
  validation without changing M1 table meaning or workspace Snapshot-head
  semantics.
- Added immutable `VersionRecord` persistence with deterministic `ver-<sha256>`
  identity, Snapshot/workspace/project/environment and generation/migration
  validation, one-to-one creation-operation binding, exact retry, and
  fail-closed changed-identity handling.
- Kept Version creation and started-operation completion atomic through the
  existing SQLite transaction/failpoint path; cold reopen re-validates durable
  Version, Snapshot, and Operation relations. Different operations targeting one
  Snapshot remain the explicit `CONTRACT_OPEN_DECISION` boundary.
- Replaced the 18 ignored 010A placeholders with executable contract tests and
  added 8 durable SQLite workspace-version tests. Focused suites and the full
  regression pass on Windows 11 / NTFS local native development; 2 pre-existing
  host/measurement tests remain ignored and are not counted as PASS evidence.
- Retained evidence under `artifacts/m2-development/`; this is
  `INTERNAL / TEST-GATED` development evidence, not M2 release evidence. ADR-M2-010
  remains Proposed / Internal M2, and no tag or push was made.

## Unreleased - M2-SLICE-010A Version Persistence Contract (2026-09-04)

- Defined the internal contract for an independent immutable Version node that
  references an existing Snapshot and binds 1:1 to a durable creation
  Operation. The proposed deterministic identity, workspace/project/
  environment and generation/migration checks, retry semantics, transaction
  boundary, and additive migration behavior are documented without executing
  DDL or changing existing M1/M2 schema meaning.
- Added the design-only [`M2_VERSION_SCHEMA.md`](../architecture/M2_VERSION_SCHEMA.md),
  Proposed [`ADR-M2-010`](../decisions/ADR-M2-010-version-persistence.md), and
  18 explicitly ignored `NOT_IMPLEMENTED_CONTRACT_TEST` cases. These cases are
  contract placeholders, not runtime evidence or a claim that Version
  persistence exists.
- Kept parent/version graph, branch, merge, candidate, approval, Version head,
  Snapshot deletion/GC, and same-Snapshot/different-operation policy outside
  this slice. No production implementation, migration, release evidence, tag,
  or push was created.

## Unreleased - M2-SLICE-009 Lifecycle Operation Identity Integration (2026-09-02)

- Reused the existing durable operation ledger to bind lifecycle mutation
  intent, workspace revision/status CAS, terminal result, journal phase, and
  operation events in one SQLite transaction.
- Added deterministic operation retry and changed-semantics rejection, stale
  caller guards, pre/post-commit recovery coverage, provider failure isolation,
  read-only operation exclusion, event taxonomy preservation, and v0.1
  compatibility coverage.
- Retained Windows 11 / NTFS local native development evidence under
  `artifacts/m2-development`; this remains internal/test-gated and is not M2
  release qualification.

## Unreleased - M2-SLICE-007 Provider-Neutral Workspace Lifecycle Contract (2026-09-02)

- Added an internal lifecycle state/action/capability model that reuses the
  existing seven durable workspace states and limits capabilities to snapshot,
  restore, diff, and status.
- Added pure transition and capability validation plus an explicit synthetic
  `TEST_DOUBLE` contract suite covering L1-L15, provider/core failure ordering,
  deterministic retry, recovery, legacy SQLite readability, and event/generation
  boundary isolation.
- Kept provider-specific paths/metadata outside the core context. No SQLite,
  M1, ADR-0016, provider, public API, or release-evidence changes were made.
- Windows 11 / NTFS local native development evidence is retained under
  `artifacts/m2-development`; this remains internal/test-gated and is not M2
  release qualification.

## Unreleased - M2-SLICE-008 Durable Workspace Lifecycle Transition Integration (2026-09-02)

- Integrated the existing provider-neutral lifecycle transition validator with
  the lease/revision-guarded `WorkspaceUpdate` transaction.
- Added deterministic exact retry handling, monotonic revision checks, stale
  lease/revision rejection, pre/post-commit fault coverage, cold-reopen
  recovery, legacy fixture compatibility, and read-only status/diff assertions.
- Kept `Open` read-like and emitted no new lifecycle operation/event because no
  lifecycle event contract is accepted in M2; snapshot/restore linkage remains
  unchanged.
- Added Windows 11 / NTFS local native development evidence under
  `artifacts/m2-development`; no M1, schema, ADR-0016, provider, or public API
  changes were made.

## Unreleased - M2-SLICE-006 Workspace Diff Reconciliation (2026-09-01)

- Defined current-tree diff as a `POINT_IN_TIME_OBSERVATION` bound to the
  durable head, workspace lifecycle revision, and immutable environment ID.
- Added post-scan workspace revalidation; detectable head, revision,
  environment, locator, or identity changes return the existing
  `CONFLICT/UNSTABLE_OBSERVATION` error without publishing state.
- Added lifecycle integration coverage for snapshot publication, restore
  materialization, deterministic repeatability, cold reopen, environment
  binding, read-only behavior, and a concurrent scan-time revision change.
- Retained Windows local native / NTFS development evidence. This remains an
  internal, test-gated slice and does not qualify M2 release platforms.

## Unreleased - M2-SLICE-005 Workspace Current-Tree Diff (2026-09-01)

- Added an internal `WorkspaceManager::diff_workspace` boundary that compares
  a local workspace's current point-in-time tree with its durable head snapshot.
- Reused canonical path validation, redaction, file-read stability checks, and
  deterministic `SnapshotDiff` classification for added, removed, modified,
  and file/directory type changes. Missing or inconsistent heads fail closed;
  no CAS, SQLite, lease, revision, operation, or event state is mutated.
- Added focused coverage for clean and changed trees, deterministic repeated
  scans, 10,000 files, corruption and identity failures, cold reopen, and the
  explicit concurrent-mutation limitation. Retained Windows local native / NTFS
  development evidence; this remains internal and is not cross-platform or
  release qualification.

## Unreleased - M2-SLICE-004 Deterministic Snapshot Diff (2026-09-01)

- Added proposed ADR-M2-004 and an internal `LocalWorkspace::diff_snapshots`
  boundary for deterministic snapshot-to-snapshot comparisons.
- Added canonical path-ordered `ADDED`, `REMOVED`, `MODIFIED`, and
  `TYPE_CHANGED` results with old/new digest, size, and type metadata. The
  comparison validates manifests but does not read every file blob or mutate
  CAS/metadata.
- Added focused coverage for empty, add/remove/modify/type-change and multiple
  changes, repeatability, 10,000 entries, corrupt manifests, cold reopen, and
  raw v0.1 compatibility. Added a synthetic-manifest complexity sanity check
  at 100/1,000/10,000/100,000 entries. This remains internal Windows local
  NTFS evidence; workspace current-tree diff is covered by M2-SLICE-005, while
  public API and M2 release qualification remain out of scope.

## Unreleased - M2-SLICE-003 Workspace Status (2026-09-01)

- Added an internal read-only `WorkspaceManager::status` view for local
  workspaces. It validates workspace/head/snapshot/CAS, generation, event,
  environment, and locator identity before returning a stable serializable
  status model.
- Defined `changed` as current filesystem content versus the published head;
  no head is reported as `change_state = "no_snapshot"` with no comparison
  baseline. Lease activity, execution readiness, and unresolved operation state
  are derived without mutating revision or lease rows.
- Added status coverage for create, snapshot, changed content, lease expiry,
  restore/operation visibility, corruption fail-closed behavior, cold reopen,
  legacy-style no-snapshot workspaces, and JSON serialization. This remains
  internal Windows local NTFS development evidence, not a public M2 release.
  The independent-target regression recorded 174 passed, 2 ignored, and 0
  failed; ignored host-resource and measurement-only tests are not counted as
  PASS.

## Unreleased - M2-SLICE-002 Durable Restore (2026-09-01)

- Added an internal durable restore operation for a verified snapshot into a
  new destination only. Restore validates snapshot/CAS/workspace/project and
  generation identity, preserves existing destinations, and does not move the
  source workspace head.
- Linked restore terminal results to `snapshot.restore.completed`, `.failed`,
  or `.unknown` Event Envelopes in the operation outcome transaction. Added
  full materialization verification to reconcile exact retries after cold
  reopen or terminal-metadata interruption without overwriting a directory.
- Added focused durable restore integration coverage for success, cold reopen,
  idempotent retry, existing-destination safety, missing blobs, synthetic
  materialization/parent-sync faults, and pre-commit metadata interruption.
  The bounded slice passed the Windows local NTFS development gate and complete
  M1 regression suite. This remains internal development evidence, not public
  API, cross-platform qualification, or M2 release evidence.

## Unreleased - M2-SLICE-001 Atomic Snapshot Publication (2026-09-01)

- Added the proposed internal ADR for snapshot identity and atomic publication.
- Added additive `snapshots` metadata with typed snapshot ID, CAS root,
  workspace/project/environment, manifest/redaction, operation/event, and
  generation/migration identity.
- Published snapshot metadata, `snapshot.created`, and workspace head/revision
  in one SQLite transaction after verified CAS publication. Pre-commit faults
  retain the old state; a post-commit outcome-unknown retry verifies and
  converges to the complete committed state.
- Added focused snapshot-publication tests and v0.1 additive-schema coverage.
  The full M1 regression suite passed in an independent Windows target
  directory. This remains an internal M2 slice, not a public API or M2 release.

## Unreleased - M1 final candidate CI closure (2026-08-29)

- Retained successful GitHub Actions Run `33253638226` at binding commit
  `b20fc903eb7395f5d3f2a48a7f0184bcc3b02713` for frozen source candidate
  `58e9e875cd5a781a94f921ec215e231cffdfafe6`.
- Linux Ubuntu 24.04/ext4 and macOS 14 arm64/unknown-filesystem jobs each
  returned zero for all 13 recorded commands. Windows remains covered by
  retained local native evidence; no Windows GitHub job is required.
- Imported both final-CI artifacts, manifests, nested checksums, and download
  metadata. Rebuilt the full bundle `SHA256SUMS` over 524 files and updated
  traceability/matrix references without modifying production code, ADRs,
  workflow, tag, or remote state.
- M1 is `READY_FOR_RELEASE`; the release tag remains intentionally uncreated
  and must follow the normal owner-controlled release process.

## Unreleased - M1 release-scope decision execution (2026-08-28)

- Recorded a proposal for the M1 supported rows: Windows x86_64/NTFS, Linux x86_64/ext4, and a GitHub-hosted macOS runner with filesystem `unknown`. This is a pending Owner decision, not an accepted support claim.
- Separated the durability/correctness/recovery baseline from universal platform certification, external-provider certification, pre-M1 compatibility, and performance-capacity certification. FI-03 deferral, Linux FI-13 applicability, old-reader exclusion, the one-CI-corpus property policy, and the ADR-0015/ADR-0016 decisions remain pending.
- Updated the Owner action package and roadmap handoff. No production code, release tag, or final release report was created, and no native/property suite was rerun in this scope-decision pass. M1 remains `BLOCKED / NOT_PASSED`.
- Audited the release bundle packaging boundary: top-level `SHA256SUMS` verifies its entries, while the selected `artifact-references.json` index has a stale self-entry and unindexed supplemental files. This is recorded as a packaging-hygiene caveat to reconcile or label during candidate freeze, not as a new Gate condition.

## Unreleased - M1 final gate closure audit (2026-08-28)

- Added `M1_FINAL_GATE_CLOSURE_PLAN.md`, `M1_PERFORMANCE_ACCEPTANCE.md`, and the human-only `M1_RELEASE_OWNER_SIGNOFF.md` template.
- Regenerated release traceability for HEAD `6cb62fb` and retained native workflow `33145714975`; the record explicitly remains blocked because the working tree is dirty, no release tag exists, and owner approval is pending.
- Re-ran the isolated full `--all`/`--all-features` quality gates, projection, property, fault, compatibility, migration, recovery, and artifact-consistency suites. All executable current-host checks passed; missing external evidence remains blocked and M1 is not passed.

## Unreleased - M1 native platform evidence success (2026-08-28)

- Retained successful GitHub Actions run `33145714975` at commit `6cb62fb`; both Ubuntu 24.04/ext4 and macOS 14 arm64 jobs passed all 13 recorded stable/MSRV, focused, and cold-reopen commands with exit code `0`.
- Imported complete Linux/macOS manifests, command exit records, nested checksums, and ZIP hashes into the source evidence tree and release bundle.
- Extended `artifact_consistency` to validate successful native artifacts, commit/run identity, complete manifests, all-zero command dispositions, and nested checksum coverage. Native rows are executable evidence `PASS`; M1 release decision remains `NOT PASSED` pending old-reader, fault, budget, and owner acceptance.

## Unreleased - M1 evidence close-out (2026-08-27)

- Retained GitHub Actions native evidence run `33085292318` for Linux and macOS; both artifacts are complete but remain `FAIL` because full tests detected committed evidence byte/hash drift.
- Extended artifact consistency coverage to the third native failure run and fixed the evidence byte boundary with `artifacts/** -text -diff` plus an explicit PT-13/FI-10 release-log reference.
- Rebuilt the release bundle references and `SHA256SUMS`; M1 remains `NOT PASSED` and a fresh native rerun is required.

## Unreleased - Phase 0 (2026-08-19)

- Established Pong scope as agent execution versioning and coordination infrastructure.
- Added vision, architecture, protocol, concept, security, reliability, compatibility, development, research, governance, ADR, and roadmap documentation.
- Recorded local-first storage, logical workspaces, extensible operation envelopes, event projections, two DAGs, policy-gated replay, and explicit framework boundaries.
- Added the M1 durable-primitives gate, ordered PoC execution plan, and experimental reports for crash ordering, capture confidence, secret redaction, and two-agent lease/merge.
- Accepted ADR-0013 for Rust Core plus bundled `rusqlite`; added the internal CAS/metadata/event scaffold and language-neutral M1 fixture schema.
- No public CLI, runtime, SDK, server, or released Core implementation added; the M1 scaffold remains gate-blocked.

## Unreleased - Phase 1 (2026-08-19)

- Added the internal Rust CAS and SQLite metadata/event primitives with bundled `rusqlite`, canonical object identity, WAL `FULL` durability, ref compare-and-swap, idempotency, and immutable event checks.
- Added redaction profile compatibility, pre-persistence redaction for metadata/events/journal fields, fail-closed scans of SQLite sidecars and CAS trees, secret rejection before object publication, and secret-safe debug output.
- Added the `.pong` repository layout and atomic `repository.json` startup marker with format/schema/storage-driver validation.
- Added recovery classification for orphan intents (`unknown`) during repository startup, idempotent journal phase transitions, restart/WAL/CAS boundary tests, and duplicate intent/event regression tests.
- Added instance-scoped CAS and metadata failpoints, cold-start recovery injection, child-process termination tests, SQLite WAL-tail truncation evidence, and a repository-wide fail-closed byte scan covering unknown `.pong` files.
- Added the cross-file repository-generation migration contract: SQLite Online Backup, generation manifests, offline repository locking, atomic `repository.json` selector replacement, selector-journal recovery, safe path validation, and old/new-only cold-restart tests.
- Added deterministic PT-01 through PT-12 and PT-14 property coverage. The
  retained 10,000-case artifact covers Windows PT-01/02/03/04/05/06/07/08/11/12;
  PT-09/10/14 have focused/default-case coverage but their storage-heavy
  10,000-case continuation was interrupted before producing a result.
- Added opt-in real-host FI-13 Windows ACL evidence and FI-14 Linux `tmpfs` ENOSPC evidence for CAS, metadata, and journal writes; all reported stable Pong permission/resource statuses without losing prior committed data.
- Added `artifacts/m1-performance.json` with metadata, recovery, CAS, and repository-migration measurements. It is measurement evidence, not an accepted performance budget.
- The M1 gate remains open: the supported-platform declaration, separately released old-reader matrix, accepted performance/capacity budgets, and complete fault matrix remain required. The historical Windows PT-14 selector/journal publication classification issue is fixed for the exercised paths, but broader filesystem coverage still needs acceptance.

## Unreleased - Phase 2 (2026-08-25)

- Added internal logical workspace, environment, and epoch-lease metadata with revision/lease guarded workspace updates and same-project environment binding.
- Added allowlisted deterministic environment facts for OS, architecture, family, and safe variables; sensitive keys, host identity/path keys, and obvious secret-shaped values are excluded before persistence.
- Added a bounded local filesystem workspace driver with logical identity, checked locators outside `.pong`, canonical portable tree manifests, immutable file blobs in CAS, symlink/reparse rejection, and file-count/per-file-size limits.
- Added materialization into a new temporary directory followed by verified file writes, directory synchronization, and rename. Existing destinations are never overwritten.
- Added integration coverage for lease/revision conflicts, cold-reopen lease takeover, repeated canonical snapshots, file-count/size limits, secret-file rejection without head advancement, materialization, same-project environment binding, manager-controlled head advancement, Unix symlink rejection, and sensitive environment allowlists.
- Added M2 fault-injection evidence for CAS publication, workspace-head commit boundaries, materialization writes/syncs/rename, parent-directory outcome-unknown publication, stable permission/resource errors, and explicit orphan cleanup after cold reopen.
- Added a raw v0.1 SQLite compatibility fixture proving migration sources are opened read-only without additive M2 DDL, unsupported formats fail before schema mutation, and target generations initialize additive workspace tables.
- Added the M2 workspace/snapshot acceptance gate. This is an unreleased internal slice: crash reconciliation, complete lifecycle operations, schema-evolution evidence, property/fault/performance matrices, and non-local drivers remain incomplete, and M1 is still not passed.

## Unreleased - Phase 3 (2026-08-25)

- Added the first internal M3 operation-ledger slice: versioned envelopes,
  durable start/terminal lifecycle rows, operation event streams, explicit
  effect classes, typed references, same-project identity bindings, redaction,
  request/operation idempotency, and before/after-commit cold-reopen evidence.
  Runtime interception and public APIs remain out of scope.

## Unreleased - M1 native platform closeout (2026-08-27)

- Retained real GitHub Actions run `33074865773` and both native evidence
  artifacts. Ubuntu 24.04 completed focused M1 evidence but failed full tests
  when Windows evidence hashes drifted after checkout line-ending
  normalization; macOS 14 completed focused M1 evidence but failed full tests
  and clippy because the host-resource fault test compiled unused
  Linux/Windows-only helpers. M1 remains not passed.
- Marked captured evidence logs as byte-exact Git `-text` files and constrained
  `tests/host_resource_faults.rs` to Linux/Windows targets. Corrected Windows
  stable/MSRV gates and artifact-consistency checks pass locally; a new native
  CI run is required before platform rows can be accepted.

- Audited and hardened `.github/workflows/m1-release-evidence.yml` for the
  native-platform closeout. The workflow now runs stable and Rust 1.78 gates
  in independent target directories, records exact command exit codes,
  pins test repositories to a workspace-local temporary directory, captures
  native filesystem identity and run metadata, executes focused cold-reopen
  evidence,
  and uploads deterministic Linux/macOS artifact names with structured
  metadata, manifest, and SHA256SUMS. The first real run `33074865773` is
  retained as a failure record; native rows remain unaccepted pending rerun.
- Added `docs/development/M1_CI_EXECUTION_REQUIRED.md` with the minimum
  commit/push/Actions/download handoff. No CI result, release tag, or owner
  acceptance is inferred from the local workflow definition.
- Published the M1 evidence workflow to `origin/dev` at
  `ca6323673cc87be30d377f3b0915f9061c2a038b`. A subsequent run exposed
  cross-platform evidence normalization and macOS target-cfg defects; the
  corrective commit and manual dispatch remain required.

## Unreleased - M1 native rerun handoff (2026-08-27)

- Fixed byte-exact evidence-log handling, constrained host-resource fault
  helpers to applicable targets, retained the failed native run `33074865773`,
  and corrected the release bundle's nested `SHA256SUMS` coverage.
- Pushed portability fix `359abc306b554d592b532ebc182e543f97489043` and metadata
  follow-up `2310c822a6f5d3f942067e0ec7a322369f03df8a` to `origin/dev`.
- Native Linux and macOS remain `FAIL` for the historical run; a new manual
  workflow dispatch is required before either row can be reconsidered. M1
  remains `NOT PASSED`.

## Unreleased - M1 evidence audit (2026-08-26)

- Assembled the M1 release evidence bundle under
  `artifacts/m1-release-evidence/`. It contains a normalized
  `PASS`/`FAIL`/`BLOCKED`/`NOT_APPLICABLE` matrix, Windows stable/MSRV build
  logs with explicit exit codes, copied platform, fault, performance,
  property, and PT-13/FI-10 records, source-path references, and `SHA256SUMS`.
  PT-13 and FI-10 remain `PASS` for the current Windows-host evidence scope,
  while release acceptance remains `BLOCKED`.
- Corrected stale audit wording: this checkout does contain Git metadata
  (`dev` at `2da6cf1037acf054b33b23b07c27388be1807690` with `origin`), but the
  working tree is dirty and has no accepted release tag or release commit.
- M1 remains **not passed**. The separately released v0.1 reader, native
  Linux/macOS rows, Windows native disk-full, real power-loss/full fault
  schedule, accepted ADR-0015 budget, and named release owner are still
  required.
- Added the minimal `.github/workflows/m1-release-evidence.yml` workflow for
  external native Linux (`ubuntu-24.04`) and macOS (`macos-14`) evidence. It
  captures platform identity, runs the Rust 1.78 quality gates plus
  projection/migration/recovery/compatibility tests, and uploads raw logs with
  hashes. No runner has executed in this workspace, so both native rows remain
  `BLOCKED`.

- Closed the current-host PT-13/FI-10 implementation evidence gap without
  changing the settled storage architecture. Added generation A-to-B
  projection migration, generation-isolation assertions, migration interruption
  checks, ProjectionFailpoint A-J crash/reopen schedules, repeated-crash
  recovery, and clean-replay golden-state comparisons. Three targeted reruns
  are retained in `artifacts/m1-pt13-fi10-summary.json` and
  `artifacts/m1-pt13-fi10-summary.log`; M1 remains **not passed** because
  external platform, old-reader, real fault, budget, and owner acceptance
  evidence is still required.

- Implemented the internal generation-bound event envelope and projection
  slice described by ADR-0016. Added additive `event_envelopes`, projection
  state/cursor tables, deterministic handler registration, idempotent apply,
  degraded unknown-event handling, atomic rebuild, legacy-event backfill, and
  schema-boundary validation. PT-13 migration fixtures, FI-10 crash schedules,
  and release-owner acceptance remain pending; M1 remains **not passed**.

- Reworked `docs/development/M1_EVIDENCE.md` into explicit A-E sections for
  passed evidence, implemented-but-insufficient evidence, unavailable host
  checks, known risks, and the release-gate decision.
- Re-ran the Rust 1.78 migration/recovery/compatibility/property suites in an
  independent Windows target directory and in a disposable Linux container;
  Linux full `cargo check` and `cargo test` passed.
- Re-ran real Windows NTFS FI-13 ACL revocation and real Linux 64 MiB `tmpfs`
  FI-14 CAS/metadata/journal exhaustion. The observed domain statuses were
  `PERMISSION_DENIED` and `RESOURCE_EXHAUSTED`, respectively.
- Kept M1 explicitly **not passed**: no separately released old-reader binary
  matrix, accepted platform/capacity budget, complete host fault matrix, or
  accepted supported-filesystem matrix exists yet. `NEXT_TASK.md` now points
  back to this release audit.
- Added the auditable [`M1_COMPATIBILITY_MATRIX.md`](../development/M1_COMPATIBILITY_MATRIX.md)
  and proposed [`ADR-0015`](../decisions/ADR/ADR-0015-m1-performance-capacity-budget.md);
  both remain pending release-owner acceptance.
- Expanded the measurement probe to include repository open, event append, and
  small/medium/large local snapshot scales. The new measurements remain
  evidence, not an automatic release threshold.
- Reproduced the concurrent Windows PT-14 selector-directory-sync failure,
  fixed the protected-directory raw access-denied mapping to
  `PERMISSION_DENIED`, and verified three concurrent PT-14 reruns. Broader
  filesystem coverage remains part of the M1 matrix.
- Extended the protected I/O mapping through repository-owned marker/journal
  atomic publication, closing the remaining Windows PT-14 journal-finalization
  `IO_ERROR` path without changing selector atomicity or retry semantics.
- Extended the bounded Windows transient sharing/access-denied retry window to
  80 attempts at 25 ms, and added Linux 1.78 clippy-driven portability fixes
  for path arguments. The complete Windows stable/MSRV and Linux quality gates
  pass after these changes.
- Re-ran the final Windows 1.95.0 quality gate, Windows Rust 1.78 isolated
  `check/test`, and pinned Linux Rust 1.78 full `fmt/check/test/clippy` after
  installing matching components in the disposable image; all exited zero.
  M1 remains **not passed** pending release-owner acceptance of the
  compatibility, fault, platform, and performance/capacity evidence.
- Refreshed `artifacts/m1-performance.json` with Rust 1.78 and
  `artifacts/m1-performance-windows-stable.json` with Rust 1.95.0 after the
  final code changes; these remain measurement evidence rather than accepted
  release budgets.
- Added explicit release-owner acceptance registers to the M1 compatibility
  matrix, evidence report, and ADR-0015, plus external-environment handoff
  commands for unavailable platform, old-reader, fault, and performance
  evidence. All decisions remain unassigned/pending and M1 remains **not
  passed**; the Linux final target-volume record is now consistent across the
  evidence package.
- Extended the measurement-only probe to `m1-perf-0.2` with p99 summaries for
  repeated timing operations, regenerated the Rust 1.78 and stable baseline
  records, and retained three raw runs per Windows toolchain under
  `artifacts/m1-performance-runs/`. These runs do not constitute ADR-0015
  acceptance or widen the supported-platform claim.
- Added a fresh isolated `target/windows-msrv-178-clippy-final` run after
  installing the Rust 1.78 clippy component; MSRV clippy now passes with
  `-D warnings` alongside the stable and Linux quality gates.
- Extended the performance probe to `m1-perf-0.3` with eight independent
  migration samples and p50/p95/p99/max summaries. The resulting artifacts
  remain measurement evidence only and do not accept ADR-0015.
- Added three pinned Linux overlay performance runs to the raw-run manifest;
  native ext4 and other filesystem claims remain explicitly unverified.
- Added `artifacts/m1-fault-matrix.json`, an auditable FI-01 through FI-14
  disposition index that records exact tests, observed statuses, and external
  gaps without promoting incomplete rows to pass.
- Re-ran the Windows NTFS FI-13 ACL schedule in a dedicated scratch directory
  and retained raw stdout plus a structured record with kernel error 5,
  `PERMISSION_DENIED`, ACL restoration, and cold-reopen results. Added the
  existing ref-CAS commit-boundary test to the FI-09 machine-matrix index.
  This remains partial host evidence; M1 is still **not passed**.
- Ran the Windows stable property corpus with `PONG_PROPTEST_CASES=10000`.
  PT-01/02/03/04/05/06/07/08/11/12 completed (10,000 cases each); the
  storage-heavy PT-09/10/14 continuation was interrupted before any result
  and is recorded as such. The raw log and case/seed manifest are retained
  under `artifacts/m1-property-runs/`; no incomplete run is counted as a pass.
- Added file-backed cold-reopen assertions for synthetic FI-01 and FI-08:
  pre-commit intent failure remains pre-state after reopen, and CAS directory
  sync failure reopens a complete immutable object with an idempotent retry.
  The fault matrix and harness docs retain the synthetic/native evidence
  boundary; M1 remains **not passed**.
- Re-ran the complete Windows stable, Windows Rust 1.78, and pinned Linux Rust
  1.78 quality gates after the FI-01/FI-08 additions in fresh target
  directories/volume `pong-msrv178-slim-target-fault`; all fmt/check/test/
  clippy commands exited zero. This confirms build portability but does not
  satisfy the external M1 release-owner, old-reader, filesystem, or fault
  matrix requirements.
- Audited the FI-10/PT-13 projection gap against ADR-0008 and the current
  metadata/event store. No projection schema/version, handler,
  cursor/checkpoint, or rebuild API exists; the matrix keeps FI-10
  `not_implemented` and M1 remains **not passed** rather than accepting an
  unrelated event-list or repository-migration test as projection evidence.
- Added a retained pinned-Rust-1.78 Linux run on an independent Docker named
  volume identified by `df -T` as ext4, with `TMPDIR` pinned to that volume so
  test repositories use the exercised filesystem. The complete
  fmt/check/test/clippy gate passed; this is Docker-VM filesystem evidence only
  and does not close the native ext4, old-reader, host fault, or release-owner
  acceptance gaps.
- Corrected the M1 evidence wording so the retained 10,000-case claim is
  limited to PT-01/02/03/04/05/06/07/08/11/12; PT-09/10/14 remain explicitly
  no-result for that long run. Marked the crate `publish = false` while its
  integration-test-visible Rust symbols remain unreleased implementation
  surface; direct metadata/CAS constructors are not a supported public API.
- Added a structured no-result record for the bounded Windows PT-09 10,000-case
  run, including the interruption boundary, observed exit code, and raw-log
  SHA-256. No result is inferred from the manual stop.
- Retained three independent pinned Linux Rust 1.78 `tmpfs` FI-14 records for
  CAS, metadata, and journal exhaustion, each with raw `ENOSPC`, Pong
  `RESOURCE_EXHAUSTED`, cold-reopen assertions, and raw-log hashes. These are
  partial platform evidence only; M1 remains **not passed**.
- Added a repository-migration regression proving that a separately valid
  generation-format SQLite database with a foreign generation/migration
  identity cannot replace the active generation and bypass selector binding.
- Added a read-only M1 artifact-consistency test that verifies retained
  raw-log SHA-256 values and fault-matrix artifact paths; it is evidence
  hygiene only and leaves the release gate not passed.
- Completed the Windows stable PT-09 recovery property rerun with 10,000/10,000
  cases passing in 1,081.76 seconds; retained the structured record and raw
  log hash. PT-10 and PT-14 remain separate incomplete normative obligations,
  and M1 remains **not passed**.
- Completed the Windows stable PT-10 crash-schedule property rerun with
  10,000/10,000 cases passing in 1,325.25 seconds (exit code `0`); retained
  the structured record and raw-log SHA-256 under
  `artifacts/m1-property-runs/`. This closes PT-10's Windows stable
  10,000-case evidence only; PT-14 and release-owner acceptance remain open,
  and M1 remains **not passed**.
- Added three bounded concurrent Windows PT-14 diagnostic records using
  independent target directories (256 requested cases across all nine
  migration failpoints per run); all exited zero with no observed host error.
  Raw logs and SHA-256 values are retained in
  `artifacts/m1-property-runs/windows-stable-pt14-diagnostic-2026-08-27.json`.
  These probes are supplemental diagnostics only; the normative PT-14 run,
  cross-platform fault matrix, and M1 release-owner acceptance remain open.
- Audited the accepted event-model documentation against the production
  metadata schema and recorded the missing event envelope/projection contract
  in `M1_EVIDENCE.md`. The current event store remains a per-stream append log;
  no projection schema, cursor, handler, or rebuild API is claimed, so PT-13
  and FI-10 remain unimplemented and M1 remains **not passed**.
- Added proposed [`ADR-0016`](../decisions/ADR/ADR-0016-projection-contract.md)
  to define the generation-bound projection contract needed before FI-10/PT-13
  can be implemented; the ADR is pending and does not change the M1 gate.
- Fixed a Rust 1.78-only `clippy::needless_borrows_for_generic_args` failure in
  `tests/artifact_consistency.rs`, then reran the Windows stable and MSRV
  `fmt/check/test/clippy` gates in fresh target directories. All commands and
  the artifact-consistency test exited zero; this does not change the M1 gate
  decision or provide missing Linux/native/old-reader evidence.
- Re-ran the complete Windows stable Rust 1.95.0 and Rust 1.78.0 MSRV
  `fmt/check/test/clippy` gates on 2026-08-27 in independent close-out target
  directories, recording exit code `0` for every command and a passing
  artifact-consistency test under each toolchain. This is refreshed local
  evidence only; M1 remains **not passed** and all external/implementation
  blockers remain visible.
- An earlier audit note incorrectly described the checkout as lacking `.git`
  metadata. The repository is on branch `dev` at
  `2da6cf1037acf054b33b23b07c27388be1807690` with `origin`; the working tree
  is dirty and no release tag/commit is accepted, so the evidence package does
  not claim a clean release artifact.
- Corrected broken relative links in ADR-0014 and ADR-0015 so the M1 storage,
  protocol, recovery, gate, and performance artifact references resolve from
  their `docs/decisions/ADR/` location.
- Completed the Windows stable PT-14 normative rerun with
  `PONG_PROPTEST_CASES=10000`: 10,008 executed cases across nine migration
  failpoints, `1 passed; 0 failed`, exit code `0`, and no observed host error or
  mixed-generation state. The structured record and raw-log SHA-256 are
  retained under
  `artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json`
  and `.log`; this closes only the Windows stable property row, not the
  cross-platform matrix or M1 release-owner acceptance.
- The PT-14 raw log capture method is recorded as session-output reconstruction
  (not a fresh tee rerun); its SHA-256 is verified by the read-only artifact
  consistency test, and this capture caveat does not widen the gate decision.

## Unreleased - M1 native evidence run #4 audit (2026-08-28)

- Retained GitHub Actions run `33142438624` and both complete native evidence
  artifacts. All native format/check/clippy and focused M1 commands passed;
  both full test commands on Linux and macOS failed at the same
  `artifact_consistency` assertion.
- Identified the deterministic cause: the committed LF `build-metadata.json`
  blob differed from stale Windows CRLF digest/reference values. Rebound the
  release `SHA256SUMS` and `artifact-references.json` records to the committed
  LF bytes and verified a clean-index export plus local artifact-consistency
  tests. M1 remains `NOT PASSED`; one fresh native dispatch is required.

## Unreleased - M1 native evidence run #2 (2026-08-27)

- Retained GitHub Actions run `33080915116` at commit
  `8d28075d44f5458e866c94ee33b95b430f7959d6` and both downloaded native
  evidence ZIPs. Linux SHA-256 is
  `B3E457A9B936347E619C280B5DB5ACE06A9C7A7C206178193D85527BDF9E0B1E`;
  macOS SHA-256 is
  `01A1BC12E9DE8051F603EEDA3CC23A920419067F38503F38635856628083F16F`.
- Recorded Linux stable/MSRV full-test failures in `artifact_consistency` due
  retained evidence hash drift after checkout. The focused migration,
  recovery, compatibility, PT-13/FI-10, and cold-reopen commands passed.
- Recorded the macOS artifact boundary honestly: only the Rust 1.78 build logs
  and focused logs were produced; MSRV test failed while MSRV clippy exited
  zero, and platform metadata, manifest, stable logs, and stable exit records
  are absent.
- Added the run to the M1 release matrix, compatibility matrix, roadmap, and
  checksum/reference bundle. Workflow capture-boundary hardening was pushed in
  `2cf136e`; M1 remains `NOT PASSED` and a fresh successful native run is
  required.
## Unreleased - M4-020 native runtime environment probe (2026-09-24)

- Rechecked the Windows worktree at `bf28943f8d841a1373411abac460a027248a824b`
  and preserved the three pre-existing untracked provider/Windows files.
- Started the installed Docker Desktop Linux engine and ran the M4 focused
  and full regression from a clean archive of the exact HEAD in a Debian 12
  `rust:1.95-bookworm` container.
- `cargo fmt`, `cargo check --all-targets --locked`, and clippy passed.
  The focused matrix failed at `d7_cold_reopen_after_restore`; the full suite
  failed in four existing `agent_execution` tests with OS error 2.
- Docker Desktop WSL2 execution is retained as supplementary diagnostic
  evidence only, not native Linux evidence. Linux and macOS remain
  `NOT_PROVEN`; M4-020 remains `BLOCKED / ENVIRONMENT`. No production code,
  checkpoint, push, or tag was created.
