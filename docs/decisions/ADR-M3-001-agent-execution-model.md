# ADR-M3-001: Agent / Task / Execution Model

- Status: Proposed / Internal M3
- Date: 2026-09-06
- Scope: M3-SLICE-001A contract design only
- Implementation: None; `PROPOSAL ONLY`

## Context

Pong already records immutable Snapshots, Versions, Workspace leases, and a
durable Operation ledger. Those records explain state and concrete actions but
do not yet define who requested work, which Agent run performed it, how child
runs relate, or how execution context can be handed off and resumed.

## Agent Identity

An Agent Identity is the durable actor identified by immutable `agent_id`.
Provider/runtime type and optional display name are descriptive metadata and
are independent from identity. Credentials and provider state are external
secret references only; no API key, token, password, private key, or prompt
transcript is stored in Core.

## Provider

Provider identifies the runtime boundary, for example `openai/codex`,
`cursor`, or an internal adapter. Provider metadata cannot redefine identity,
lease authority, operation identity, Version identity, or state transitions.
No provider integration is implemented by this ADR.

## Task

A Task is an immutable work identity and mutable observed coordination state.
It has `task_id`, project scope, goal/context reference, state, and audit
metadata. A Task may have many Executions across independent Workspaces.
Pong does not schedule or assign work in this slice.

The proposed Task States are `created`, `running`, `blocked`, `waiting`,
`completed`, `failed`, `cancelled`, and `interrupted`. They are distinct from
Execution States.

## Execution

An Execution is one concrete run of one Agent for one Task. It has immutable
`execution_id` and `task_id`, `agent_id`, provider metadata, optional parent,
Workspace, base Version, current Version, its own state, outcome, and timing.
One Execution may produce many Operations and Versions and must never be
treated as a Version alias.

Execution States are `created`, `running`, `paused`, `completed`, `failed`,
`interrupted`, and `unknown`. `unknown` means an outcome is not confirmed
after crash or an uncertain persistence boundary.

## SubAgent

A child or SubAgent is represented by another Execution with an explicit
`parent_execution_id`. The parent must already exist, the edge is immutable,
and cycles are rejected at any depth. Child failure does not silently complete
or fail the parent.

## Execution Graph

The Execution Graph contains Agent, Task, and Execution nodes. Only the
parent-child edge is frozen in this slice. Handoff, resume, dependency,
Checkpoint, and Operation ownership are independent relations and must not be
encoded by overloading `parent_execution_id`.

## Workspace Attachment

An Execution may attach to one explicit Workspace, and a Task may involve
multiple Workspaces through its Executions. Parallel writable Executions use
independent Workspaces by default. Any writable attachment must use the
existing Workspace lease and revision CAS; graph membership grants no write
authority.

## Version Attachment

`base_version_id` names the explicit Version from which an Execution starts;
`current_version_id` names its latest explicitly observed Version. Neither is
derived from Snapshot Head, Version Head, creation order, or timestamps.
Parallel Executions may share a base Version without becoming a linear Version
chain.

## Operation Ownership

An Operation attributed to an Execution has exactly one immutable Execution
owner. The ownership association is proposed separately from the existing
`operation_id` and must preserve existing request, causal, generation,
redaction, event, lease, and revision semantics. No operation column or
mapping table is added in 001A.

## Handoff

A Handoff transfers ownership and context from one Execution to another while
preserving Task identity, Workspace reference, base/current Version
references, Operation history, and optional Checkpoint. It has its own
`handoff_id`, source/target Executions, reason, source Version, and redacted
context reference. The target does not inherit authority without revalidating
the Workspace lease and scope.

## Checkpoint

A Checkpoint is an immutable recovery anchor referencing Task, Execution,
Workspace, Version, and an optional Operation cursor. Adapter state, task
context, environment fingerprint, and replay/approval boundaries are opaque
or redacted references. A Checkpoint never copies or mutates Version data.

## Rollback

Rollback to Version, Rollback to Checkpoint, Rollback Task, and Rollback
Execution are distinct control decisions. Rollback never deletes or rewrites
Versions, Operations, Executions, or already-observed irreversible effects.
The implementation of rollback is outside this ADR.

## Resume

Resume uses an explicit Version or Checkpoint and preserves the failed or
interrupted Execution as history. A new attempt or handoff context may be
created; the prior record is never overwritten and no fake Version is created.

## Failure

Provider failure, process crash, timeout, child failure, parent failure, and
Core persistence failure must not become phantom success. Provider output is
not durable completion. An uncertain commit remains `unknown` until cold
reopen and deterministic retry establish the durable result.

## Recovery

After reopen, Pong must be able to identify Task, Execution, Agent, Workspace,
base/current Version, last Operation, child Executions, and state. Missing or
inconsistent references fail closed. Existing M2 operation and transaction
semantics remain authoritative for durable actions.

## Concurrency

Parallel Executions are independent graph nodes and may branch from one base
Version. They are not serialized into a Version linear chain. Shared writable
Workspace use is not implied and requires a separate contract.

## Lease

Executions cannot bypass the existing Workspace lease, epoch, expiry, or
revision CAS. A handoff target must acquire the applicable lease before a
write. Stale execution context uses the existing deterministic conflict or
integrity model.

## Context Transfer

Transferable context may reference Task goal, base/current Version, relevant
Operations, Checkpoint, known failures, and decisions. It must be redacted or
opaque and is not Agent memory, Version content, or a secret-bearing prompt.

## Security

Agent identity and provider metadata are non-secret descriptors. Secrets are
external references only. Core must apply existing redaction and project
scope checks to all future Task, Execution, Handoff, Checkpoint, and ownership
records.

## Compatibility

This ADR changes no M1 `v0.1.0` meaning, Workspace Head, Snapshot schema,
Version identity, Version parent, Version Head, Operation identity, SQLite
schema, or ADR-0016 projection contract. It adds no provider, CLI, SDK, UI,
Branch, Merge, Candidate, Approval, or Agent State implementation.

## Alternatives

- Treating Execution as Version was rejected because activity and immutable
  state have different identity, retry, and recovery semantics.
- Treating Agent Identity as a provider was rejected because one provider may
  host many durable identities and identities may move between providers.
- Encoding Handoff and dependency as parent edges was rejected because these
  relations have different direction, lifecycle, and recovery meaning.
- Requiring one shared Workspace for a Task was rejected because parallel
  writable work requires lease isolation.

## Open Decisions

- exact durable entities, fields, indexes, and migration strategy;
- project versus broader scope for Agent and Task registries;
- nullable Workspace/Version attachment rules by Execution kind;
- concrete Operation ownership and event representation;
- Handoff, resume, dependency, and Checkpoint storage relations;
- transition authorization and owner takeover policy;
- provider capability and external secret-reference vocabulary; and
- retention, deletion, and external-effect compensation rules.

## Decision

Adopt this document as the proposed internal contract for M3-SLICE-001A.
Only the parent-child Execution relation is frozen. All implementation and
schema choices remain subject to a later bounded slice.

`M3-SLICE-001A = CONTRACT_READY`.
