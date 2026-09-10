# M3 Execution Graph

**Status:** `IMPLEMENTED` / `INTERNAL` / `TEST-GATED`
**Slice:** M3-SLICE-001B - Agent Execution Core
**Implementation:** The parent Execution graph and its Agent/Task/Execution
nodes are durable in SQLite. The graph is bounded to the four additive
entities implemented by `MetadataStore`; future relation types remain out of
scope.

## Graph Boundary

The M3 Execution Graph records why work was requested and how concrete Agent
runs relate. It is separate from the M2 Version Graph:

```text
Execution Graph                         Version Graph

Task -> Execution -> Operation          Version -> parent Version
          |              |              Version -> Snapshot
          +-> Workspace  +-> Event      Workspace -> Version Head
          +-> Version references
```

Execution nodes describe activity and context. Version nodes describe
immutable logical state. An execution edge never implies a Version parent
edge, and a Version edge never implies Agent ownership.

## Nodes

### Agent Identity

An immutable `agent_id` identifies the actor. Provider/type and optional
display name are descriptive metadata. Credentials are external secret
references and never graph payloads.

### Task

`task_id` identifies the requested unit of work. A Task may have multiple
Executions, including parallel attempts, retries, resumes, and handoffs. Task
state is independent from every Execution state.

### Execution

An Execution identifies one concrete run with `execution_id`, `task_id`,
`agent_id`, provider metadata, optional Workspace and Version references,
parent execution, state, outcome, and timestamps. It may own many Operations
and may produce many Versions.

## Frozen Edge: Parent Execution

`parent_execution_id` is the only graph edge frozen by this slice. Its rules
are:

1. The parent must already exist.
2. Parent and child must belong to the same project and compatible Task
   context.
3. An execution cannot parent itself.
4. Adding an edge must reject any cycle, including multi-level cycles.
5. The edge is immutable after creation.

The graph supports arbitrary finite nesting, not only one child level.

```text
E1
|- E2
|  |- E4
|- E3
```

## Independent Relations

Handoff, resume, dependency, checkpoint, and Operation ownership are
separate relations. They must not be encoded by overloading
`parent_execution_id`:

- **Handoff:** ownership/context transfer from one Execution to another;
- **Resume:** a new attempt continues from an explicit Version or Checkpoint;
- **Dependency:** one Execution waits for another result;
- **Checkpoint:** a recovery anchor references Task, Execution, Workspace, and
  Version; and
- **Operation ownership:** each attributed Operation has one Execution owner.

These relations are proposals only and remain independently queryable if
implemented later.

## Parallelism

Parallel Executions are siblings or otherwise independent nodes, not a linear
Version chain:

```text
             E1 / V101
            /
T1 -- base V100
            \
             E2 / V102
```

Each writable Workspace has its own M2 lease and revision guard. Independent
Workspaces are the default for parallel writable work. Shared writable
Workspace policy requires a separate contract.

## Handoff Edge Semantics

A Handoff links `from_execution_id` to `to_execution_id` for one unchanged
Task. It carries a reason, observed source outcome, source Workspace, base and
current Version references, optional Checkpoint, and a redacted context
reference. It does not change Agent Identity, rewrite Operations, create a
Version, or become a parent edge.

The target Execution must explicitly revalidate Workspace lease, project,
Version, generation, migration, and redaction scope before writing. A target
does not inherit authority merely by receiving context.

## Version and Operation Links

Version references are explicit IDs only:

- `base_version_id` is the start anchor;
- `current_version_id` is the latest observed result; and
- produced Versions remain immutable M2 records.

Operations retain their existing `operation_id`, request identity, causal
fields, event stream, and transaction semantics. A future ownership
association cannot change those identities or infer ownership from timestamps,
latest rows, Version Head, or provider output.

## State Boundaries

Task States:

`created`, `running`, `blocked`, `waiting`, `completed`, `failed`,
`cancelled`, `interrupted`.

Execution States:

`created`, `running`, `paused`, `completed`, `failed`, `interrupted`,
`unknown`.

`unknown` means that a crash or persistence boundary leaves the outcome
unconfirmed. No graph query may infer completion from a provider's response,
an Operation row alone, or a child result.

## Failure and Recovery

The graph must remain explainable across provider failure, process crash,
timeout, child failure, parent failure, Core persistence failure, and unknown
post-commit outcomes. Cold reopen must expose durable Task, Execution, Agent,
Workspace, Version, Operation, and child relationships or fail closed on an
inconsistent reference.

Rollback preserves all graph and Version history. Resume creates a new
explicit attempt or handoff context; it never rewrites an interrupted or
failed Execution. External irreversible effects are not silently compensated.

## Authorization and Lease

Graph membership is not authority. A child, handoff target, or provider
callback must still satisfy the existing project binding and Workspace lease
and revision checks. Agent capabilities are declarations interpreted by the
integration boundary, not credentials or automatic permission grants.

## Implemented Schema Boundary

The logical entities are:

```text
AgentIdentity(agent_id, provider, display_name, secret_ref, project_scope)
Task(task_id, project_id, goal_ref, state, timestamps)
Execution(execution_id, task_id, agent_id, parent_execution_id,
          workspace_id, base_version_id, current_version_id, state, outcome,
          timestamps)
Checkpoint(checkpoint_id, task_id, execution_id, workspace_id, version_id,
           operation_cursor, context_ref, timestamps)
Handoff(handoff_id, task_id, from_execution_id, to_execution_id, reason,
        source_version_id, target_context_ref, timestamps)
ExecutionOperation(execution_id, operation_id)
```

The implemented tables are `agent_identities`, `tasks`, `executions`, and
`execution_operations`. They are created additively for new and legacy-opened
databases, while the v0.1 source database is opened read-only during
generation migration. Operation ownership is separate from the existing
Operation identity. No Checkpoint or Handoff table is created in this slice.

## Compatibility

The proposal does not change M1 `v0.1.0`, M2 Workspace Head, Snapshot,
Version identity, Version parent, Version Head, Operation identity, SQLite
schema meaning, or ADR-0016. It introduces no provider, CLI, SDK, UI, Branch,
Merge, Candidate, Approval, or Agent State implementation.

## Status

`M3-SLICE-001B = PASS / INTERNAL / TEST-GATED`.

A development-only scale sanity completed entity counts of 1, 100, and 1,000
and iterative graph depths of 10, 100, and 1,000. This is implementation
evidence only and does not establish a release performance budget.

The graph implementation is covered by durable integration tests, including
multi-level ancestry, cycle rejection, parallel children, ownership,
recovery, retry, and scope validation. Handoff, dependency, Checkpoint,
Rollback, and Resume remain `OPEN / FUTURE CONTRACT`.
