# M3 Handoff / Checkpoint / Rollback Contract

**Status:** `IMPLEMENTED / INTERNAL / TEST-GATED`
**Slice:** M3-SLICE-002D - Rollback Result / Materialization Core
**Implementation:** The additive durable relation tables, local Snapshot
materialization, guarded dual-head publication, retry, and recovery paths are
implemented and covered by real SQLite-backed tests. Provider integration, CLI,
SDK, and release evidence remain out of scope.

## Boundary

This contract defines the domain meaning and safety boundaries for transferring
Execution responsibility, recording stable recovery anchors, rolling work back
without deleting history, and starting a new attempt from an old state.

The terms are deliberately distinct:

| Entity | Meaning | Existing authority |
| --- | --- | --- |
| Task | What work is requested | Existing durable `tasks` row |
| Execution | One Agent run for one Task | Existing durable `executions` row |
| Version | One immutable logical state | Existing M2 Version graph |
| Checkpoint | Immutable reference to a stable Version | Durable core implemented |
| Handoff | Transfer of responsibility/context between Executions | Durable core implemented |
| Rollback | Audited recovery action whose history is retained | Prepared/completed durable result and local materialization implemented |
| Resume | New Execution attempt from a Version or Checkpoint | Durable core implemented |

No relation below is encoded by overloading `parent_execution_id`, Version
parentage, `Workspace.head`, Version Head, or `operation_id`.

## Existing Model Audit

The contract is constrained by the implemented M1/M2/M3 model:

- `Execution.base_version_id` is the existing explicit per-Execution start
  anchor. `TaskRecord` has no baseline field today; no baseline column is added
  in this slice.
- `Execution.current_version_id` is an explicit observed result and is not
  derived from Snapshot Head or Version Head.
- `Workspace.head` remains the Snapshot root digest required by M1.
- `Workspace.version_head_id` is an independent logical Version selection and
  is updated only by its existing lease/revision-guarded path.
- Version identity, immutable `parent_version_id`, Snapshot binding, and
  creation Operation identity remain unchanged.
- The existing Operation ledger records action intent/outcome, but does not by
  itself provide a queryable immutable Handoff, Checkpoint, Rollback, or Resume
  relation with its own target, scope, and recovery state.
- Workspace writes require the existing lease and revision CAS. Handoff never
  transfers a lease implicitly.

## Handoff

A Handoff is a durable relation from source Execution `E1` to target Execution
`E2` for the same Task. It transfers responsibility and bounded context; it
does not create a Version, copy Version bytes, change Task identity, rewrite an
Operation, or create a parent-child edge.

The proposal contains at least:

- immutable `handoff_id`;
- `task_id`, `from_execution_id`, and `to_execution_id`;
- source Workspace, base Version, and current Version references;
- optional `checkpoint_id`;
- reason and observed source outcome as redacted/opaque references;
- requester/actor Agent and, where applicable, actor Execution references;
- status, request identity/digest, and timestamps.

### Handoff status and transition

The relation status is separate from Task and Execution State. The bounded
proposal uses `requested`, `accepted`, `completed`, `failed`, `cancelled`, and
`unknown`. A failed or unknown Handoff is never treated as a completed
transfer. The durable transition is all-or-nothing:

```text
no handoff
  -> requested
  -> accepted
  -> completed
```

At an uncertain commit boundary, reopen observes either the prior complete state
or the complete new state. It never exposes `E1` as completed or `E2` as an
active owner without a durable Handoff record.

The conservative first implementation may commit `accepted` and target
Execution activation in one transaction. `completed` is reserved for a
confirmed transfer result; exact status transition details remain an
implementation decision, not a reason to weaken the invariant.

### Handoff preconditions

Before acceptance, Core must verify:

1. Task, source Execution, and target Execution exist.
2. Both Executions belong to the same Task and project.
3. The source state permits transfer. `running`, `paused`, or explicit
   `interrupted` handoff-ready state is allowed; `completed` and `failed` are
   not. `unknown` requires explicit reconciliation and is not auto-transferable.
4. The target is a fresh `created` Execution for takeover. A running or
   terminal target fails closed.
5. All Workspace and Version references are present, scoped, generation-valid,
   and migration-valid when supplied.
6. A same-workspace target has a valid lease before it writes. Handoff itself
   does not grant lease authority.
7. The request identity and canonical digest are either a new request or an
   exact retry. Changed semantics return the existing deterministic conflict.

### Handoff ownership and graph rules

The actor is recorded and must be authorized by the existing Agent/Execution
identity boundary. The minimum candidates are the source Agent, a parent
Execution's Agent, or an explicitly authorized controller at the integration
boundary. A Task owner is not assumed because the current Task model has no
owner field. A second authorization framework is not introduced.

Handoff edges are separate from parent edges. The source's parent remains the
source's parent, and the target's declared parent remains unchanged. A target
cannot be both the accepted target of a live Handoff and the source of a cycle.
The bounded contract rejects `E1 -> E2 -> E1` and serializes concurrent
requests. Multiple simultaneous Handoffs beyond one active transfer per source
and target remain an open policy decision.

### Handoff context

Context is a bounded set of durable references to Task goal, source/target
Executions, base/current Versions, relevant Operations, optional Checkpoint,
known failures, and decisions. It is not Version content, Agent memory, a
prompt transcript, or a provider secret. Context payloads must be redacted or
opaque handles and must be integrity-checked on read.

### Handoff scenarios

- Quota exhaustion: source becomes `interrupted` or `paused`; target is a new
  Execution for the same Task and may proceed only after Handoff and lease
  checks.
- Agent crash: source remains `interrupted` or `unknown` according to the
  existing durable boundary; target takeover cannot rewrite it to `completed`.
- Voluntary handoff: source records its observed state and target receives only
  explicit references.
- Nested handoff: a child `E2` handing to `E4` retains both the original
  parent-child lineage and the independent Handoff edge.

## Checkpoint

A Checkpoint is a durable, immutable, recoverable anchor that points to an
already complete Version. It never copies or mutates Version content and never
changes Version identity, Version parent, Snapshot bytes, Workspace Head,
Version Head, or Operation identity.

The proposal contains:

- immutable `checkpoint_id`;
- `task_id`;
- an optional `execution_id` depending on the eventual scope policy;
- `workspace_id` and `version_id`;
- optional Operation cursor/reference;
- reason/label references and actor identity;
- request identity/digest and `created_at`.

### Stability and preconditions

Checkpoint creation must verify that the Version exists, its Snapshot and
creation Operation are durable, all workspace/project/environment/generation/
migration bindings are valid, and no unresolved recovery state makes it unsafe.
Missing, corrupted, failed, or unknown Version state fails closed. A
Checkpoint is either absent or complete after a crash; a dangling Checkpoint
reference is never healthy.

User-created, Agent-created, and automatic safety checkpoints are all allowed
as future trigger sources. Trigger policy is not implemented here.

### Scope decision boundary

Task-scoped and Execution-scoped Checkpoints have different meanings:

- a Task Checkpoint is a stable anchor for the whole Task and must not imply
  that every parallel Execution is at that state;
- an Execution Checkpoint is a stable anchor for one Execution's workspace and
  context and must not grant Task-wide rollback authority.

Whether Core supports Task scope, Execution scope, or both is intentionally
`OPEN DECISION`. The schema proposal retains a scope discriminator rather than
silently choosing one for implementation convenience. C7 and C8 preserve this
decision as explicit contract coverage.

## Rollback

Rollback is an auditable recovery operation, not deletion, history rewrite, or
`git reset`. A target Version or Checkpoint is a historical source. Historical
Versions, Operations, Executions, and external-effect observations remain
durable.

### Targets

The contract names four target kinds without conflating them:

1. immutable Version;
2. Checkpoint;
3. Task baseline (not yet stored by the current Task model);
4. Execution baseline (`Execution.base_version_id`).

The target must resolve to one healthy Version before mutation. A Task baseline
cannot be inferred from the oldest row, timestamps, Workspace Head, or Version
Head.

### Result semantics (closed by M3-SLICE-002C)

Rollback restores the authorized Workspace physical tree to the target
Version's existing Snapshot. After successful verification and publication:

```text
physical Workspace tree = target Snapshot content
Workspace.head           = target Snapshot root digest
Workspace.version_head_id = target Version
```

Rollback itself creates no Version and records `result_version_id = NULL`.
Historical Versions, Snapshots, Operations, Executions, Checkpoints, Version
identity, and parentage remain unchanged. A later Resume creates a new
Execution and may then create a new Version with the rollback target as its
explicit parent. These semantics are implemented for the local workspace
driver by `WorkspaceManager::rollback_local` and remain internal/test-gated.

### Scope and isolation

- Default Execution rollback affects only the explicitly authorized target
  Execution and its bound Workspace.
- It never automatically changes sibling/parent Executions, their Workspaces,
  other Tasks, historical Versions, or historical Operations.
- Task-level rollback is a separate explicit scope that may enumerate several
  affected Workspaces/Executions and must be auditable and transactional.
- Selective/component rollback and cross-workspace rollback are future
  contracts requiring ownership and adapter scope; current behavior must fail
  closed rather than guess.

### Preconditions and authorization

Core verifies target Version health, Workspace/Task scope, generation,
migration, environment, current Execution authority, and a valid Workspace
lease for any write. The actor reuses Agent Identity/Execution identity and
existing lease authority. A child Execution does not gain project-wide rollback
authority by graph membership.

## Resume

`resume_from(version)` and `resume_from(checkpoint)` always create a new
Execution attempt for the same Task. The old failed, interrupted, or unknown
Execution remains immutable history. The new Execution explicitly records its
base Version and starts with no inferred current Version. Resume does not
reopen or overwrite the old Execution and does not create a fake Version.

Resume may be combined with Handoff, but the relations remain distinct:

```text
E1 interrupted -> Handoff -> E2 running
E2 interrupted -> Resume from C1 -> E3 created/running
```

The new attempt must pass the same project, Workspace, Version, generation,
migration, redaction, and authorization checks as any new Execution. Exact
retry returns the same durable attempt; changed source/checkpoint/agent
semantics fail deterministically.

## Task Baseline

The current `TaskRecord` has no `baseline_version_id`. This slice therefore
does not add or infer one. The existing `Execution.base_version_id` is a valid
Execution baseline. A future Task baseline must be an explicit immutable
relation or field with its own request, scope, migration, and retry contract;
the storage choice remains `OPEN DECISION`.

## Execution Baseline

`Execution.base_version_id` is immutable and explicit. It is the only current
baseline authority for one Execution. `current_version_id` records the latest
explicit result and never replaces the baseline. Rollback or Resume may name
the baseline, but cannot rewrite it.

## Version Relationship

Checkpoint and rollback references point at existing immutable Versions. They
do not alter Version identity, parentage, Snapshot binding, or the Version
Head. A new Version after recovery must use the existing M2 Version creation
and parent contract; no parent is inferred from a rollback target or timestamp.

## Workspace Scope

Workspace writes remain lease/revision guarded. Handoff does not transfer a
lease. Rollback of one Execution is Workspace-local by default. A target
Workspace different from the source must be explicit, same-project and
authorized, and remains a future cross-workspace policy; no implicit project-
wide mutation is allowed.

## Parallel Agent

Parallel Executions may share a base Version and must use independent
Workspaces for concurrent writes by default. E1 rollback does not affect E2 or
E3. A Task-level rollback is the only candidate for coordinated multi-agent
change and requires explicit scope, audit, transaction, and authorization.

## Multi-Level Agent

Parent-child Execution edges remain immutable and independent of Handoff,
Checkpoint, Rollback, and Resume. A child handoff never reparents the target;
an E2 -> E4 transfer preserves E1 -> E2 and the target's declared parent.

## Failure / Recovery Contract

Each future durable mutation has an old-or-new boundary:

| Operation | Pre-commit failure | Post-commit uncertainty | Forbidden result |
| --- | --- | --- | --- |
| Handoff | no relation; source/target unchanged | complete relation and matching state transitions | source completed without handoff; target active without record |
| Checkpoint | no checkpoint | complete immutable checkpoint | dangling or half checkpoint |
| Rollback | target/current state unchanged | complete rollback record, target materialization, and matching heads | partial materialization; mismatched head; dangling head; phantom success |
| Resume | no new Execution | complete new Execution attempt | half Execution; old Execution overwritten |

Cold reopen must classify any uncertain record using existing recovery/error
semantics. Unknown is explicit; provider output never proves success.

## Idempotency and Conflict

Every request has a durable operation/request identity and canonical digest.
Exact retries return the original durable Handoff, Checkpoint, Rollback, or
Resume result. Reusing a request identity with changed source, target, scope,
checkpoint, Version, actor, or action returns deterministic conflict. Concurrent
requests serialize; no random conflict or duplicate durable entity is allowed.

## Security

No API keys, OAuth tokens, cookies, passwords, private keys, prompt
transcripts, or provider secrets may enter these records. Context, reasons,
labels, and external payload references pass through existing redaction and
remain opaque. References do not grant authorization.

## Legacy and Compatibility

The proposal is additive-only. v0.1 readers continue to ignore future tables;
v0.1 schema meanings, M1 events, ADR-0016 projection semantics, Snapshot Head,
Version identity/parent/Head, Operation identity, and current M2 migration
rules remain unchanged. No legacy baseline, checkpoint, handoff, or rollback
row is inferred.

## Additive Schema

The implementation creates these additive tables with `CREATE TABLE IF NOT
EXISTS`; it does not alter the M1/M2 tables or their meanings:

- `handoffs`: identity, Task/source/target references, source state and
  Version/Workspace references, optional Checkpoint, actor, reason/context
  references, status, request digest, and timestamps;
- `checkpoints`: identity, Task/scope discriminator/optional Execution,
  Workspace/Version references, optional Operation cursor, actor, labels/reason
  references, request digest, and immutable timestamps;
- `rollback_records`: identity, Task/Execution/actor, target kind/reference,
  target Workspace scope, initiating Operation, pre/post state references,
  resulting Snapshot/Version references when available, status, and recovery
  metadata. `result_version_id` remains nullable because rollback itself does
  not create a Version; the local result materialization is implemented and
  verified before completion.
- `resume_attempts`: identity, source Execution, Version/Checkpoint source,
  new Execution, initiating Operation, request digest, status, and timestamps.

The existing Operation ledger remains the initiating/audit action and is linked
to these records. It is insufficient as the sole relation because it cannot
express immutable multi-target scope, independent query identity, checkpoint
multiplicity, or old/new recovery state without overloading Operation result
payloads.

## Test Contract

`tests/handoff_checkpoint_rollback_contract.rs` remains the open-decision
index; its cases are explicitly ignored and are not PASS evidence. Runtime
coverage is in `tests/handoff_checkpoint_rollback.rs` (48 passing cases) and
`tests/rollback_result.rs` (30 passing cases),
using the real SQLite-backed MetadataStore. The focused runtime suite covers
durable creation, exact retry, redaction, generation/migration binding,
pre/post-commit failpoints, stale lease/revision rejection, cold reopen, and
legacy-compatible additive initialization.

## Open Decisions

- Task-scoped, Execution-scoped, or dual-scope Checkpoints;
- whether Task baseline is a field, an immutable relation, or a designated
  baseline Execution;
- selective/component rollback ownership and cross-workspace policy;
- exact Handoff authorization and Task-owner/controller policy;
- automatic checkpoint trigger and retention/deletion policy;
- checkpoint deletion and referenced Version retention;
- multiple simultaneous Handoffs and relay semantics;
- Resume request identity and whether `resume_attempts` is independently
  durable;
- whether task-level rollback can coordinate parallel Workspaces; and
- exact event taxonomy, if any, after an event contract is accepted.

## Status

`M3-SLICE-002D = PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::rollback_local` records a prepared intent, validates the
target Version/Checkpoint and CAS manifest, materializes a verified sibling
tree, replaces the local Workspace tree, then publishes `Workspace.head`,
`Workspace.version_head_id`, revision, and rollback completion in one guarded
SQLite transaction. Completion failures remain retryable and converge to the
old-or-new state. The relation-only `create_rollback` API records `prepared`
until a real local materialization completes; non-local providers and public
APIs remain out of scope.
