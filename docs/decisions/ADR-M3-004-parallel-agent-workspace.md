# ADR-M3-004: Parallel Agent Workspace / Reconciliation

- **Status:** `Proposed / Internal M3`
- **Date:** 2026-09-09
- **Scope:** M3-SLICE-003A contract design only
- **Implementation:** none; no DDL, production code, provider, or scheduler

## Context

M3 can represent multiple Agent Executions for one Task, while M2 already
provides Workspace isolation, leases, revision CAS, Snapshot/Restore/Diff, an
immutable Version Graph, and an explicit Version Head. Parallel execution must
not collapse those identities or infer authority from completion order.

## Goals

- Define ownership and write authority for parallel Executions.
- Permit isolated parallel Workspaces sharing a Task/base Version.
- Reuse M2 reconciliation and failure/recovery semantics.
- Define deterministic conflicts for shared mutable scope.
- Preserve Handoff, Checkpoint, Rollback, Resume, and nested Execution
  boundaries.

## Non-Goals

No runtime implementation, schema/DDL, scheduler, worker pool, Agent
orchestration, Git/Remote provider, Provider Registry, Branch, Merge, Rebase,
Candidate, Approval, Agent State, Memory, Skill, Automation, CLI, SDK, UI, or
automatic conflict resolution is included. Merge remains **FUTURE**.

## Workspace Ownership

Mode A (one writable Execution -> one Workspace) is the safest shape. Mode B
(multiple Executions -> one Workspace) is rejected by default and would require
an explicit future collaboration mode plus serialized lease/revision writes.
Mode C (one Task -> multiple Workspaces) is **RECOMMENDED**: parallel Agents
share Task and, when explicitly selected, `base_version_id`, but not a
writable Workspace. Ownership is the explicit Execution attachment combined
with a valid Workspace lease.

Only the current lease holder may write, snapshot, restore, rollback, or make a
Version Head selection in that Workspace. Diff/status are read-only. Handoff
does not implicitly transfer a lease. Reconciliation reports instability; it
does not resolve it.

## Parallel Execution

Each Execution remains an independent graph node. E1/W1, E2/W2, and E3/W3
may run concurrently, including concurrent Snapshot, Restore, Diff, Rollback,
and Resume operations when their Workspaces are unrelated. A same-Workspace
writer race returns deterministic Workspace/Lease/Revision Conflict.

## Base Version

Multiple Executions may share an explicit `base_version_id`. Their successful
Versions are independent children of that base. No Merge, Branch, or linear
ordering is inferred. Same Snapshot plus different Operation remains the M2
open decision and is not closed here.

## Workspace Head

`Workspace.head` remains the Snapshot digest and is isolated per Workspace. A
publication on W1 cannot modify W2/W3. Rollback updates only the target
Workspace's physical tree and Snapshot/Version heads under its existing
materialization contract.

## Version Head

Version Head is an explicit logical selection distinct from Snapshot Head and
Version parentage. Selection requires the existing scope, lease, and revision
guards. Concurrent selection of different heads is a deterministic Version Head
Conflict; completion order and last-write-wins are not authority.

## Lease and Revision

Leases belong to Workspaces and are independent across W1/W2. Every mutation
validates owner/epoch/expiry and expected revision. Stale values fail closed.
Reads do not alter either field.

## Conflict Model

Workspace, Revision, Lease, Version Head, Handoff, Rollback, and Reconciliation
Conflict are stable deterministic outcomes. A conflict never becomes success
through a random retry, provider message, or “last completed” rule.

## Reconciliation

M2-SLICE-006 is authoritative: capture identity/head/revision/environment, scan,
re-read, and compare. Any required change yields `UNSTABLE_OBSERVATION`/Conflict.
Independent Workspaces are not conflicts merely because they share Task or
base Version. Diff remains read-only and emits no operation/event.

## Handoff

Handoff links explicit source/target Executions in one Task/project and carries
bounded redacted context. The target must revalidate scope and acquire a lease.
Parent-child ancestry, Handoff, and Workspace ownership remain separate; child
descendants do not migrate automatically. Competing takeovers are Handoff
Conflict.

## Checkpoint

Checkpoints are immutable references. Parallel C1/C2 anchors remain independent
and rollback of C1 cannot mutate C2 or its Version.

## Rollback

Rollback is Workspace-local by default and preserves all Version, Snapshot,
Execution, Operation, and Checkpoint history. It does not create a Version;
Resume creates a new Execution and a later Snapshot/Version. Rollback of W1
cannot affect W2/E2/V201. Same-Workspace rollback races are deterministic
Rollback/Lease/Revision conflicts.

## Resume

Resume preserves the source failed/interrupted Execution and creates a new
Execution with explicit base/current references. Sibling Executions remain
unchanged.

## Multi-Level Agent

Parent Execution graph, Handoff graph, Workspace attachment, and rollback
scope are independent. Nested children may use distinct Workspaces and a
handoff does not reparent or migrate descendants.

## Task Semantics

Task state is distinct from Execution state. One failed and one completed
Execution may leave the Task running/active. Completion and aggregation require
an explicit policy and remain open; the last completed Execution is not enough.
Provider quota is an opaque interruption/failure reason, not a Core provider
rule. Partial completion is valid.

## Failure Boundary

Agent/process/provider failure, lease expiry, stale revision, Snapshot/Restore/
Rollback/Handoff/Resume failure, and scan mutation expose complete old state,
complete new state, or a deterministic conflict/unknown result. No phantom
success is allowed. Crash reopen preserves interrupted/unknown state and
reconciles using existing durable boundaries.

## Recovery

Cold reopen must independently classify each Execution and Workspace. Parallel
success and failure coexist. A scan that observes mutation is unstable; it
does not return stale data as stable.

## Security

Only opaque/redacted references cross Core boundaries. Secrets and raw prompts
are excluded. Cross-project Workspace/Version/Checkpoint/Rollback sharing is
forbidden by default.

## Legacy

M1 v0.1.0 readers and M2 compatibility remain unchanged. No synthetic
parallel relation is inferred for legacy repositories.

## Schema Proposal

Prefer existing Workspace, Execution, Operation, lease, and revision records.
The recommended ownership model needs no new durable entity. Reconciliation is
transient. A future shared-writer mode, ownership transfer, or durable
observation audit may require a separately accepted relation and migration. No
DDL is authorized here.

## Alternatives

- Shared writable Workspace by default was rejected because it permits lost
  updates and unexplainable diffs.
- Last-completed Version Head was rejected because completion order is not
  explicit selection authority.
- A global Task Workspace was rejected because it prevents safe parallel
  writes.
- Encoding Handoff as parent ancestry was rejected because the relations have
  different lifecycle and recovery meaning.
- Automatic Merge/Rebase was rejected; both remain FUTURE.

## Open Decisions

Shared Workspace collaboration; ownership transfer; Version Head arbitration;
cross-Workspace/task coordination; selective rollback; reconciliation
persistence; automatic conflict resolution; Task status/completion policy;
resource quota/fairness; provider fairness; scheduler; Branch; Merge; Rebase.

## Risks

Future shared writers could weaken lease assumptions. Ambiguous Version Head
selection could create logical divergence. Durable reconciliation records could
accidentally become a second event system. Provider context could leak secrets
or be mistaken for Core authority.

## Decision

Adopt the above as the internal proposal for M3-SLICE-003A. It freezes no
runtime behavior beyond preserving existing M1/M2/M3 authorities and marks the
recommended isolated-Workspace model for a later implementation contract.

## Status

`M3-SLICE-003A = CONTRACT_READY` only after the companion proposal documents,
ignored contract tests, and limited validation commands pass.
