# Exploration and Route Model

**Date:** `2026-10-03`

**Status:** Design slice only. No production code, schema, or Protocol v1.0
change is authorized by this document.

## Decision Summary

Pong already has the durable primitives needed to preserve and fork agent
work:

- `Task` identifies the long-lived unit of work.
- `Execution` identifies one concrete Agent run and is the correct Attempt
  primitive.
- `Workspace` plus lease/revision/CAS isolates writable materializations.
- `Snapshot` records filesystem state.
- `Version` records immutable state lineage through `parent_version_id`.
- `Checkpoint` anchors recovery without copying Version bytes.
- `Resume` creates a new Execution from an explicit Version or Checkpoint.
- `Handoff` transfers context between Executions without changing Version
  identity.
- `Rollback` restores a selected historical state without deleting history.
- `Materialization` restores a Version into an explicitly authorized target
  Workspace.
- `Operation` and `Event` preserve durable intent, ownership, retry, and
  recovery evidence.

The missing capability is not another filesystem, Version, Execution, or
Checkpoint system. The missing capability is an upper-layer relation that
names an exploration route and ties its executions, source state, candidate
state, evaluations, and selection decisions together.

The design therefore keeps the following boundary:

```text
Version = immutable code/state lineage
Route   = one named exploration path within one Exploration
```

`Route` is not a Git branch, and `Version` is not renamed or repurposed as a
branch. Multiple Routes may reference the same root Version and then produce
independent child Versions in separate Workspaces.

## Domain Relations

```text
Task
 |
 +-- Exploration
      |
      +-- Route A -----------------------------+
      |     |                                  |
      |     +-- Execution A1..An               +-- Candidate A
      |     +-- Checkpoints                    |     +-- Evaluation[]
      |     +-- root/terminal Version refs     |
      |                                        +-- Selection (optional)
      |
      +-- Route B
            +-- Execution B1..Bn
            +-- Checkpoints
            +-- root/terminal Version refs
            +-- Candidate B
                  +-- Evaluation[]
```

The existing graphs remain separate:

```text
Execution graph: Task -> Execution -> Operation/Event
Version graph:   Version -> parent Version -> Snapshot
Route relation:  Exploration -> Route -> Execution/Version refs
```

An Execution parent edge is not a Route edge, a Handoff edge, or a Version
parent edge. A Version parent edge is not proof of Agent ownership or route
membership. The future Route relation must make these references explicit.

## Minimal Concepts

### Exploration

An Exploration is the durable scope containing alternative routes for one Task.
It should have a stable `exploration_id`, `task_id`, actor/creation metadata,
and an explicit context or purpose reference. It is not a scheduler, search
algorithm, or evaluator.

Exploration should become a durable aggregate because route listing,
candidate comparison, and selection need a stable scope that cannot be
reconstructed safely from timestamps or the current Task state. It does not
need a second Task state machine. Task state remains independent: a failed
route does not fail the Task, and an active route does not silently complete
the Task.

### Route

A Route is a durable exploration-path relation with the smallest useful
identity:

```text
route_id
exploration_id
root_version_id (or an explicit source Checkpoint/Version reference)
terminal_version_id (nullable)
created_by / created_at
purpose or reason (opaque, redacted reference)
```

Route status should start small. `active`, `paused`, `failed`, `completed`,
and `abandoned` are sufficient intent states if a later implementation needs
explicit route control. A route's failed/completed outcome should be derived
from its terminal Execution and durable outcome whenever possible, rather than
duplicated as an independently mutable fact. `abandoned` or an explicit pause
may require a route-level decision because neither is implied by an Execution
row.

Route membership must be explicit. It must not be inferred from Version
parent traversal, latest Workspace Head, Agent identity, or creation order.

### Attempt and Outcome

`Attempt` is an upper-layer label for an existing `Execution`:

```text
Route -> Execution[]
Attempt ~= Execution
```

No `AttemptRecord` is needed. Execution already carries Agent, Task, parent
execution, Workspace, base/current Version, state, outcome, timestamps,
Operation ownership, and cold-reopen semantics.

Route outcome is an aggregate view over its executions and terminal Version
references. Provider failure remains an Execution/Operation failure; Pong must
not invent a second Failure entity or infer route success from provider output.

### Candidate

A Candidate is the evaluation subject for one route state. It should be a
small durable relation rather than a second Version-like entity:

```text
candidate_id
exploration_id
route_id
version_id
created_at
```

The Version remains immutable and authoritative. A Candidate is useful when a
route has several meaningful terminal or intermediate states, when multiple
evaluations must refer to one state, or when selection must remain auditable.
If the first implementation only evaluates route terminal Versions, the
Candidate relation may be created as an explicit projection of that pair; it
must not copy Snapshot or Version data.

### Evaluation

Evaluation is a durable relation from a Candidate to an evaluator and
structured evidence:

```text
evaluation_id
candidate_id
evaluator / actor
status
evidence or reference
reason
created_at
```

The initial contract should not require `score: f64`. Evidence may represent
tests, human approval, model review, benchmark output, or a multi-dimensional
external result. Any score remains optional metadata until its scale,
ordering, precision, and provenance are stable enough for a Core contract.

### Selection

Selection is a durable decision record, not a global winner field:

```text
selection_id
exploration_id
candidate_id
actor / evaluator
reason
evidence or reference
created_at
```

Pong records who selected which Candidate and why. It does not compare scores
automatically, choose a universal best route, or overwrite prior selections.
Different tasks may optimize safety, speed, cost, performance, or minimal
change, so selection must remain scoped to an Exploration and auditable.

## Route Fork and Backtracking

The existing Version, Checkpoint, Resume, Workspace, and Materialization
semantics compose the required fork behavior:

```text
V0
 |
 V1 ---- Checkpoint C1
 |
 V2  (Route A, later fails)

from C1/V1:
  create Route B
  create a new Execution B1
  acquire Workspace B
  resume/materialize the explicit source state
  publish V3 with parent_version_id = V1

Version graph:
          V1
         /  \
 Route A V2   V3 Route B
```

Required invariants:

1. Route A, its failed Execution, Checkpoint, and Versions remain durable.
2. Route B has a new Route identity and a new Execution identity.
3. The source Version and Snapshot are immutable.
4. V3 names V1 explicitly as its Version parent; no parent is inferred from
   Workspace Head or route creation order.
5. Route A and Route B use separate writable Workspaces for concurrent work.
6. A single Workspace has one materialized head at a time; history and routes
   can coexist, but two physical trees cannot occupy one Workspace head.
7. Resume, Handoff, Rollback, and Materialization retain their existing
   ownership, lease, revision, generation, and recovery checks.

This is route forking, not Git branch creation. No branch ref, merge model,
working-tree clone, or automatic search is implied.

## Recovery, Handoff, and Parallelism

### Recovery

Route recovery is a query composition:

```text
Route -> latest explicit Execution -> latest Checkpoint/Version -> Resume
```

An interrupted or unknown Execution remains history. Resume creates a new
Execution linked to the explicit source. Route inspection must not reopen or
rewrite the failed Execution, and it must not infer success from a provider
callback or a latest timestamp.

### Handoff

The existing Handoff relation is reused. A Handoff can transfer context from
one route Execution to another Agent, or a new route can be created from a
Checkpoint before handoff. No second Exploration-specific handoff mechanism
is needed. Handoff does not transfer Workspace lease authority; the target
must acquire its own valid lease.

### Rollback and Materialization

Rollback restores an explicit Version or Checkpoint into one authorized
Workspace and preserves all route history. It creates no Version. A later
Execution can publish a new child Version. Materialization similarly creates
target-local filesystem state from an immutable source Version and keeps
source/target Workspace heads distinct.

### Parallel Routes

Parallel route execution is composable with existing Workspace isolation:

```text
Route A -> Workspace A -> lease/revision A
Route B -> Workspace B -> lease/revision B
```

The Version DAG may share a base Version, while Workspace leases serialize
writes independently. Concurrent writes to one Workspace remain a lease
conflict and are not reinterpreted as route merging.

## Reuse Audit

| Question | Decision | Reason |
| --- | --- | --- |
| Exploration as durable entity? | Yes, eventual minimal aggregate | Stable scope for routes, candidates, evaluations, and selection |
| Route as durable entity? | Yes, eventual relation | Route identity/purpose/fork source cannot be inferred from Version DAG |
| Candidate as independent heavy entity? | No | Small relation to Route and immutable Version is sufficient |
| Attempt as new entity? | No | Reuse Execution |
| Outcome as new entity? | No initially | Reuse Execution outcome and Operation result; expose route projection |
| Evaluation as durable relation? | Yes, when candidates exist | Multiple evidence records and evaluators need stable references |
| Score in Core schema? | No initially | Preserve structured evidence; avoid fixed numeric semantics |
| Best-state / winner? | Selection relation only | Actor/evaluator chooses within an Exploration; no global winner |
| Version parent/children for route lineage? | Reused for state lineage | Route membership remains a separate explicit relation |
| Workspace isolation for parallel routes? | Reused | Existing lease/revision/CAS semantics are the authority |
| Checkpoint + Resume for fork? | Reused | New Execution from explicit source; old history preserved |
| Handoff for route transfer? | Reused | Existing context/ownership relation is sufficient |
| Rollback/materialization? | Reused | Existing explicit source and target semantics preserve history |

## Protocol Impact

External Agent Protocol v1.0 remains frozen. This design does not add
commands, fields, or wire resources. Future protocol candidates may include:

```text
create_exploration
create_route
fork_route
list_routes
get_route
create_candidate
record_evaluation
select_candidate
compare_candidates
```

Those are candidates for a separately versioned protocol slice. They must not
be smuggled into v1.0, and they must reuse the same Core validation and
ownership rules as the existing commands.

## Classification

Current status is:

```text
Existing bottom-up fork/recovery capability: COMPOSABLE
Exploration/Route semantic relations: DOMAIN_MODEL_GAP, not implemented
Candidate/Evaluation/Selection: DESIGN ONLY
Protocol gap: NOT ESTABLISHED; v1.0 remains unchanged
Schema gap: NOT YET ESTABLISHED; defer DDL until relation contract is tested
Implementation gap: expected after this design slice
```

The absence of route identity and evaluation/selection relations is an
upper-layer domain gap, not evidence that Pong needs duplicate filesystem,
Version, Execution, Snapshot, or Checkpoint infrastructure.

## Implementation Plan

### E1: Exploration and Route Relations

- Add only the minimal durable scope/route relations after the contract is
  approved.
- Reuse Task, Agent, Execution, Version, Checkpoint, and Workspace IDs.
- Define route fork source and explicit Execution membership.
- Test creation, idempotency, scope validation, route listing, and cold reopen.
- No Protocol v1.0 change; Core/internal first.

### E2: Route Fork and Lifecycle

- Add fork-from-Version/Checkpoint composition using existing Resume and
  Materialization paths.
- Derive failure/completion from Execution outcome unless an explicit route
  decision is required.
- Test failed Route A plus independent Route B, immutable history, separate
  Workspaces, concurrent leases, and cold reopen.
- No Git branch, merge, scheduler, or provider adapter.

### E3: Candidate and Evaluation

- Add Candidate as a relation to an immutable Version and Route.
- Add evidence-first Evaluation records with optional opaque score metadata.
- Test multiple evaluators, repeated evidence, scope, immutability, and
  persistence.
- Do not force a numeric score or automatic ranking.

### E4: Selection and Comparison

- Add auditable Selection records scoped to an Exploration.
- Expose comparison/listing as read models over Versions, Candidates, and
  Evaluations.
- Test multiple selections, changed criteria, provenance, and no global winner.
- Keep materialization/resume explicit and actor-controlled.

### E5: External Protocol Candidate

- Only after Core relations stabilize, define a separate protocol proposal or
  versioned extension.
- Map commands to existing authorization, leases, revisions, and durable
  idempotency.
- No v1.0 field or capability changes in E1-E4.

### E6: Real Agent Integration

- Separate from this design slice and from route-domain work.
- Reuse the Runtime Identity Adapter and provider-neutral transport boundary.
- Do not add Codex, Claude, or Cursor-specific Core entities.
- Run real provider E2E only under a separately approved experiment boundary.

## Non-Goals

This document does not implement or authorize:

- Git branches, merge, commit, tracking, or repository replacement;
- automatic search, ranking, scheduling, or winner selection;
- provider-specific adapters or Runtime managers;
- a duplicate Snapshot, Version, Execution, Workspace, Checkpoint, or
  recovery system;
- Protocol v1.0 changes;
- changes to `easyCode`, `D:\bs\seekwd`, or existing provider/Windows files.

## Design-Slice Result

```text
Exploration: DESIGN ONLY / minimal durable aggregate recommended
Route: DESIGN ONLY / durable relation required for explicit membership
Candidate: DESIGN ONLY / small Route-Version relation
Evaluation: DESIGN ONLY / evidence-first relation
Selection: DESIGN ONLY / scoped decision relation
Backtracking: COMPOSABLE
Parallel routes: COMPOSABLE with separate Workspaces
Handoff: REUSED
Resume: REUSED
Rollback: REUSED
Version DAG: REUSED
Protocol v1.0: UNCHANGED
Provider-specific adapter added: NO
Production code changed: NO
New schema: NO
```
