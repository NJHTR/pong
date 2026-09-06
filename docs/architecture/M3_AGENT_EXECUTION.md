# M3 Agent / Task / Execution Contract

**Status:** `PROPOSAL ONLY` / Internal M3
**Slice:** M3-SLICE-001A - Agent / Task / Execution Contract
**Implementation:** None. This document defines a contract and data-model
proposal only; it does not add production types, SQLite tables, migrations,
providers, a CLI, an SDK, or a UI.

## Purpose

Pong needs to describe why work was requested, who performed it, which
execution instance was running, which workspace was touched, and which
immutable Version or Operation resulted. These are different concepts and
must not be collapsed into one record.

| Concept | Meaning | Existing or proposed boundary |
| --- | --- | --- |
| Agent Identity | Who can perform or cause work | Proposed M3 identity; no credentials stored |
| Provider | Runtime or framework that hosts an agent | Descriptive adapter metadata, separate from identity |
| Task | Why the work exists and its durable context | Proposed M3 work unit |
| Execution | One concrete run of one Agent for one Task | Proposed M3 execution node |
| Operation | A concrete durable action during an Execution | Existing M3 operation ledger; ownership link proposed |
| Workspace | Where filesystem state is materialized | Existing M2 workspace and lease boundary |
| Version | An immutable logical state | Existing M2 Version graph and Version Head |

An Agent Identity can own many Executions. A Task can have many parallel,
retried, resumed, or handed-off Executions. One Execution can produce many
Operations and Versions. An Execution is never a Version alias.

## Agent Identity

The minimum identity is:

- immutable `agent_id`;
- descriptive `provider` / runtime type; and
- optional non-sensitive `display_name`.

`agent_id` identifies a durable actor across process runs. A process/session
incarnation is execution metadata, not a new Agent Identity. Provider and
identity remain separate, so `codex-main` may use `openai/codex` while another
identity uses the same provider.

Credentials are never fields in this model. API keys, OAuth tokens,
passwords, private keys, prompts containing secrets, and raw provider state
must remain outside Core behind an external secret reference. A secret
reference is an opaque identifier and does not grant authority by itself.

## Provider

Provider metadata describes the runtime boundary only. It may state adapter
type, declared capabilities, and an external run reference, but it cannot
change Core identity, Version identity, Operation identity, Workspace lease
rules, or task state transitions. No Git, remote, or real Agent provider is
implemented in this slice.

## Task

A Task is an immutable work identity plus mutable observed coordination state.
It contains a `task_id`, project binding, goal/context reference, timestamps,
and the current Task State. It may be related to multiple Workspaces through
its Executions; it must not require all Executions to share one physical
Workspace.

The proposed Task State vocabulary is intentionally limited to:

`created`, `running`, `blocked`, `waiting`, `completed`, `failed`,
`cancelled`, `interrupted`.

Task State is not Execution State. A Task is completed only by an explicit
durable decision; an individual failed or interrupted Execution does not
silently complete or fail the Task. Scheduling and assignment remain outside
Pong.

## Execution

An Execution is one actual run and has at least:

- immutable `execution_id` and `task_id`;
- `agent_id` and provider descriptor;
- optional `parent_execution_id`;
- optional `workspace_id`;
- optional explicit `base_version_id` and `current_version_id`;
- its own Execution State; and
- start/end and outcome metadata.

Nullable Workspace and Version references represent a deliberate execution
that has not yet attached to a workspace or produced a Version. They are not
permission bypasses and must never be inferred from the latest workspace or
Version row.

The proposed Execution State vocabulary is:

`created`, `running`, `paused`, `completed`, `failed`, `interrupted`,
`unknown`.

`unknown` is used after a crash or an uncertain persistence boundary. It is
never inferred as `completed` from provider output, timestamps, or a child
result. State transitions remain explicit and fail closed.

## SubAgent and Execution Graph

An Execution may create any finite depth of child Executions. A child points
to one existing `parent_execution_id`; the parent must exist before the child
is durable. The parent relation is acyclic and is not a Task hierarchy, a
Handoff, a dependency, or a Version parent edge.

```text
Task T1
|
+-- Execution E1 / Agent A / Workspace W1
|   +-- Execution E2 / Agent B / Workspace W2
|   |   +-- Execution E4 / Agent C / Workspace W4
|   +-- Execution E3 / Agent A / Workspace W3
```

Parallel children may share the same `base_version_id`, but they do not form
a linear Version chain. Each writable Workspace still requires the existing
M2 lease and revision guards.

## Workspace Attachment

An Execution may attach to at most one writable Workspace at a time. The
attachment records the explicit Workspace identity; the physical locator is
not an Agent or Execution identity. A writable Execution must hold the
existing Workspace lease and use its revision CAS. Multiple Agents therefore
use independent Workspaces by default. Shared writable Workspace policy is a
separate contract.

## Version Attachment

`base_version_id` is the explicit Version from which an Execution begins;
`current_version_id` is the latest explicitly observed Version produced by
that Execution. Neither field is derived from Snapshot Head, Version Head,
creation time, or Version graph order. An Execution may produce multiple
Versions, and parallel Executions may branch from one base Version without
mutating the Version graph.

The existing Version identity, parent relation, Snapshot Head, and Version
Head semantics remain unchanged. Handoff, rollback, and resume never create a
fake Version merely to represent control flow.

## Operation Ownership

Every Operation attributed to an Execution must have exactly one immutable
Execution owner. The proposed ownership association is separate from the
existing Operation identity and does not rewrite `operation_id`. It must
preserve the existing project, request, causal, generation, redaction, lease,
and revision checks. An Operation from a child Execution remains attributable
to the child while the parent relation is queried separately.

The association is a future schema/data-model proposal only. M3-SLICE-001A
does not add an `execution_id` column, a mapping table, or operation events.

## Handoff

Handoff transfers execution ownership and context, not Version identity or
Task identity. A Handoff proposal contains:

- immutable `handoff_id`;
- `from_execution_id` and `to_execution_id`;
- reason and observed outcome;
- explicit source Workspace, base Version, and current Version references;
- optional Checkpoint reference; and
- a redacted context-transfer reference.

The source and target Executions belong to the same Task. The target receives
the source context by explicit references and does not recreate the Task. A
Handoff edge is not stored in `parent_execution_id`; parent-child and
handoff relations remain independently queryable. A target may write only
after obtaining the applicable Workspace lease.

## Checkpoint

A Checkpoint is an immutable recovery anchor, not a copied Version. It
references a Task, Execution, Workspace, Version, and (when applicable) an
Operation cursor. It may carry an opaque, redacted adapter-state reference,
task context, environment fingerprint, and replay/approval boundary as
defined by the existing Checkpoint concept. The bytes and hidden model state
are not interpreted by Core.

Creating a Checkpoint does not mutate Version identity, Snapshot bytes,
Workspace Head, or Operation identity. A Checkpoint is valid only when all
required referenced records are durable and scope-compatible.

## Rollback and Resume

Rollback targets must remain distinct:

- Rollback to Version selects an immutable Version as the recovery source.
- Rollback to Checkpoint resumes from its explicit anchor.
- Rollback Task or Rollback Execution is a control decision over future work,
  not deletion of durable history.

Rollback never deletes or rewrites Versions, Operations, Executions, or
observed external effects. A new attempt may create a new Version from the
chosen source. Resume after a failure or rollback preserves the failed
Execution as history and starts a new explicit attempt or handoff context;
it never overwrites the failed record.

## Failure Boundary and Recovery

The following boundaries are explicit:

- provider failure is not Core success;
- Agent process crash or timeout produces `interrupted` or `unknown` based on
  the durable boundary, never inferred completion;
- a failed child does not silently complete its parent;
- parent failure does not erase child history;
- Core persistence failure leaves no phantom successful Execution, Operation,
  Version, or Checkpoint; and
- an uncertain post-commit result is resolved by cold reopen and deterministic
  retry using existing ledger semantics.

After reopen, Pong must be able to identify the Task, Execution, Agent,
Workspace, base/current Version, last Operation, child Executions, and
durable state. Missing or inconsistent references fail closed.

## Concurrency and Lease

Parallel Executions are independent graph nodes. They may share a base
Version, but each writable Workspace mutation is serialized by the existing
Workspace lease and revision CAS. An Execution cannot bypass lease ownership
because it is a child, a handoff target, or a provider callback. Stale
execution context is rejected deterministically using the existing conflict
and integrity error model.

## Context Transfer

Context transfer is explicit and bounded. The transferable context may name
the Task goal, base/current Version, relevant Operations, Checkpoint, known
failures, and decisions. It must use redacted durable references or opaque
external payload handles. Context is not Version content, Agent memory, or a
secret-bearing prompt transcript.

## Compatibility

This proposal preserves:

- M1 `v0.1.0` meanings and old-reader behavior;
- `Workspace.head` as Snapshot root digest;
- Version identity, parent, and Version Head semantics;
- existing Operation identity and event contracts;
- Snapshot and SQLite schema meaning; and
- ADR-0016 projection semantics.

No DDL, migration, provider registry, public API, CLI, SDK, UI, Branch,
Merge, Candidate, Approval, or Agent State behavior is part of 001A.

## Open Decisions

- exact durable entity schema and migration strategy;
- whether Task and Agent registries are project-local or broader;
- whether Workspace and Version attachments are nullable for all Execution
  kinds or restricted by execution type;
- the concrete Operation ownership association and event representation;
- Handoff, resume, dependency, and checkpoint relation storage;
- task/execution transition authorization and owner takeover policy;
- provider capability and external secret-reference vocabulary; and
- retention, deletion, and external-effect compensation rules.

## Status

`M3-SLICE-001A = CONTRACT_READY`.

This is an internal proposal. No production implementation or runtime
evidence is claimed.
