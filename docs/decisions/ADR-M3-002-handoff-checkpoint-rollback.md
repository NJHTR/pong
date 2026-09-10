# ADR-M3-002: Handoff / Checkpoint / Rollback / Resume Contract

- Status: `Implemented core / Internal M3 / Test-gated`
- Date: 2026-09-07
- Scope: M3-SLICE-002A contract plus bounded M3-SLICE-002B durable core
- Implementation: Additive relation tables and MetadataStore APIs are
  implemented. No provider, CLI, SDK, UI, or release integration is included.

## Context

M3-SLICE-001B provides durable Agent Identity, Task, Execution, parent
Execution lineage, explicit Workspace/Version references, Operation ownership,
lease/revision authority, failure states, retry, and cold reopen. Long-running
multi-Agent work still needs a precise contract for responsibility transfer,
stable recovery anchors, history-preserving rollback, and new attempts from
old state.

The existing M2 model imposes hard boundaries: `Workspace.head` is a Snapshot
root digest; `Workspace.version_head_id` is a separate logical Version
selection; Versions and their parent edges are immutable; Operations have
stable identity; and Workspace writes require the existing lease/revision CAS.

## Goals

- Define Handoff, Checkpoint, Rollback, and Resume without conflating Task,
  Execution, Version, Snapshot, Workspace, or Operation.
- Define lifecycle, scope, authorization, idempotency, concurrency, failure,
  and cold-reopen invariants.
- Preserve parallel and nested Execution isolation.
- Provide an additive schema proposal without executing it.
- Name open product decisions instead of silently choosing them.

## Non Goals

This ADR does not implement Handoff, Checkpoint, Rollback, Resume, Branch,
Merge, Rebase, Candidate, Approval, Agent State, Memory, Skill, Automation,
Git/Remote Provider, external Agent API, CLI, SDK, UI, DDL, migration, or a
new authorization framework. It does not change M1 v0.1.0, ADR-0016,
`Workspace.head`, Version identity/parent/Head, Operation identity, or the
Repository architecture.

## Handoff

Handoff is an independent directed relation from `from_execution_id` to
`to_execution_id` for the same Task. It transfers responsibility and bounded
context, never Version bytes, Task identity, Operation identity, Workspace
lease, or parent-child lineage.

The future record includes `handoff_id`, Task/source/target references, source
Workspace/base/current Version references, optional Checkpoint, reason,
redacted context reference, actor identity, request digest, status, and
timestamps. Source and target must be same-task/same-project; source must be
running/paused/explicitly interrupted and target must be a fresh created
Execution. Unknown source state requires explicit reconciliation. The target
must acquire a valid Workspace lease before writing.

The relation rejects cycles such as E1 -> E2 -> E1 and serializes concurrent
requests. Parent Execution edges remain unchanged. At most one active transfer
per source/target is the conservative implementation boundary; relay and
multiple simultaneous Handoff policy remains open.

## Checkpoint

Checkpoint is an immutable durable anchor to an already complete, healthy
Version. It contains a stable identity, Task reference, optional Execution
scope, Workspace/Version references, optional Operation cursor, actor, reason/
label references, request digest, and timestamps. It never copies or mutates
Version content or changes Snapshot Head, Version Head, parentage, or Operation
identity.

The common safety rule is closed: missing, corrupted, failed, unknown,
wrong-scope, wrong-generation, or uncommitted Version references fail closed.
User, Agent, and automatic safety triggers are future trigger sources.

Whether Checkpoint scope is Task, Execution, or both is intentionally open.
Task scope means a whole-task stable anchor and does not imply every parallel
Execution is at that state. Execution scope means one Execution's workspace
and context and does not grant Task-wide rollback authority.

## Rollback

Rollback is an independent audited recovery action. It targets an immutable
Version, Checkpoint, Task baseline, or Execution baseline. It never deletes or
rewrites history, changes Version identity/parent, or automatically affects
sibling Executions, other Workspaces, or other Tasks.

The default scope is one authorized Execution and its Workspace. Task-level,
selective/component, and cross-workspace rollback require a separate explicit
scope and remain future policy. A target must be healthy and satisfy Task,
Workspace, project, environment, generation, migration, lease, and authority
checks.

Rollback is not a direct move of `Workspace.head` or Version Head. A later
implementation must express restored current state through a complete new
Snapshot/Version or another accepted new-state record while retaining the
historical target. Whether the result selects `version_head_id` is open and,
if accepted, must use the existing selector path.

Rollback requires its own durable record because the generic Operation ledger
cannot independently express target kind, affected scope, pre/post state,
old-or-new recovery status, and exact rollback retry without overloading an
Operation result payload. The initiating Operation remains linked.

## Resume

Resume from a Version or Checkpoint always creates a new Execution attempt for
the same Task. The source failed/interrupted/unknown Execution remains intact.
The new attempt explicitly records its base Version and does not infer a
current Version. Exact retry returns the same attempt; changed source,
checkpoint, actor, or scope under one request identity conflicts.

Resume may be combined with Handoff, but the Handoff and Resume relations are
independent and parent lineage is never rewritten.

## Task Baseline

The current Task schema has no baseline field. This ADR does not add one or
infer a baseline from timestamps, oldest rows, Snapshot Head, or Version Head.
`Execution.base_version_id` is the existing per-Execution baseline. A future
Task baseline may be an immutable field, relation, or designated baseline
Execution; the storage choice is open and must have its own scope and retry
contract.

## Execution Baseline

`Execution.base_version_id` is explicit and immutable. `current_version_id` is
the latest explicit result. Rollback and Resume may reference the baseline but
cannot rewrite it.

## Version Relationship

Checkpoint and rollback reference existing immutable Versions. A recovery
result that creates a new Version must use the existing M2 Version creation and
parent contract; no parent is inferred from rollback target or creation time.
Version Head selection is independent and remains unchanged unless a later
accepted contract explicitly selects the new result.

## Workspace Scope

Workspace mutations remain lease/revision guarded. Handoff does not transfer a
lease. Execution rollback is Workspace-local by default. Cross-workspace and
in-place materialization policy remain future contracts; no project-wide
mutation is implied.

## Parallel Agent

Parallel Executions may share a base Version but use independent Workspaces by
default. E1 rollback cannot automatically rollback E2/E3 or their Versions.
Only an explicit Task-level rollback may coordinate several Workspaces, and it
must enumerate scope and preserve a complete audit trail.

## Multi-Level Agent

Parent-child Execution is a separate immutable relation. E2 handoff to E4 does
not reparent E2, E4, or the original E1 lineage. Handoff cycles are rejected.

## Failure Boundary

The four future mutations obey old-or-new visibility:

- Handoff failure leaves source/target prior state; no target owner without a
  durable Handoff.
- Checkpoint failure leaves no row or a complete immutable row; no dangling
  Version reference.
- Rollback failure leaves current state unchanged or a complete new restored
  state; no partial Workspace head or phantom success.
- Resume failure leaves no new Execution or a complete new attempt; the old
  Execution is never overwritten.

Unknown is explicit and is resolved by cold reopen/reconciliation, not by
provider output.

## Recovery

Pre-commit failures expose the old state. Post-commit acknowledgement loss is
resolved by cold reopen to the complete durable row/result. Missing or
inconsistent references fail closed. Exact retry recognizes the same request
and never duplicates a Handoff, Checkpoint, Rollback, or Resume attempt.

## Idempotency

Each request uses a durable Operation/request identity and canonical digest.
Exact same semantics return the same result. Changed source, target, scope,
Version, Checkpoint, actor, or action under one identity returns deterministic
key reuse/conflict. Concurrent identical calls converge; different calls do
not randomly win.

## Concurrency

Existing SQLite transaction boundaries and Workspace lease/revision CAS remain
authoritative. Relation rows, state transitions, and initiating Operation
outcomes must be committed atomically where the accepted future contract
requires them. Parallel sibling Executions remain isolated.

## Authorization

No second authorization framework is introduced. Current source Agent, parent
Execution Agent, or an explicitly authorized controller are candidate Handoff
actors. Rollback requires current Execution authority plus the relevant
Workspace lease. Task-owner authority is open because Task has no owner field.
All requests record actor identity and authorization basis.

## Security

Only opaque/redacted references are persisted. API keys, OAuth tokens, cookies,
passwords, private keys, provider secrets, prompt transcripts, and raw Agent
memory are forbidden. Opaque references do not grant authority.

## Legacy

The proposal is additive-only. v0.1 readers need not understand future
relation tables, and migration creates no synthetic Handoff, Checkpoint,
Rollback, Resume, or Task baseline. M1/ADR-0016 and M2 Snapshot/Version/
Operation semantics remain unchanged.

## Implemented Additive Schema

The implementation creates these additive entities with `CREATE TABLE IF NOT
EXISTS`; no existing M1/M2 table or meaning is changed:

- `handoffs` for source/target transfer, actor, context, status, and retry;
- `checkpoints` for immutable Task/scope/Execution/Workspace/Version anchors;
- `rollback_records` for target, scope, pre/post state, result, and recovery;
- `resume_attempts` for source anchor, new Execution, request identity, and
  retry, if the Execution creation path cannot itself carry the relation.

Existing Operation remains the initiating audit action, not the sole domain
record. Event taxonomy and any rollback result linkage remain open; the
implemented relation rows intentionally do not invent those semantics.

## Alternatives

- Reuse Operation result payloads only: rejected because relation identity,
  scope, checkpoint multiplicity, and recovery status become unqueryable or
  mutable.
- Encode Handoff as `parent_execution_id`: rejected because ownership transfer
  is not ancestry and would corrupt the frozen Execution graph.
- Treat Checkpoint as a copied Version: rejected because it duplicates state
  and changes Version identity/retention semantics.
- Move Version Head for rollback: rejected as an implicit interpretation of
  current state; selection requires its own accepted contract and lease/CAS.
- Delete later Versions during rollback: rejected because rollback preserves
  audit history and parallel isolation.

## Open Decisions

- Task vs Execution vs dual Checkpoint scope;
- Task baseline storage;
- rollback result and Version Head selection;
- in-place vs new-destination rollback materialization;
- selective/cross-workspace ownership;
- Handoff authorization/controller semantics;
- automatic checkpoint, retention, and deletion policy;
- multiple simultaneous Handoffs and relay semantics;
- ResumeAttempts independent identity and event taxonomy;
- Task-level parallel rollback transaction shape.

## Risks

- Choosing Checkpoint scope prematurely could grant Task-wide authority to one
  Agent.
- Treating rollback as pointer movement could violate Snapshot/Version
  separation and create a dangling or misleading head.
- Reusing Operation payloads alone could lose durable relation identity during
  recovery.
- Shared Workspace handoff without lease serialization could produce two
  writers.
- Provider context leakage could persist secrets or unverifiable Agent memory.

## Status

`M3-SLICE-002B = IN_PROGRESS / INTERNAL / TEST-GATED`.

The durable core is implemented without changing M1/M2 meanings. Rollback
materialization, resulting Snapshot/Version creation, Version Head selection,
and the other listed open decisions remain unaccepted and unimplemented. This
ADR does not authorize release evidence, tag creation, or push.
