# E2 Route Fork / Lifecycle Result

Date: 2026-10-03

## Scope

E2 composes the existing Exploration/Route, Version, Checkpoint, Workspace,
Materialization, Execution, Handoff, Resume, Rollback, and Operation APIs.
It does not change Protocol v1.0 or Version parent scope semantics.

## Verification

Command:

```text
cargo test --test exploration_e2 --locked
```

Result: PASS, 2 tests.

Tests:

- `e2_route_fork_lifecycle_preserves_local_lineage_and_history`
- `e2_route_statuses_and_idempotency_are_explicit`

## Durable flow

The focused lifecycle test records the following real durable relations:

```text
Route A source = Version V1
Route B source = Checkpoint C1 -> Version V1
Workspace A: V1 -> V2
materialize V1 -> Workspace B
Workspace B: V3 (local root) -> V4
```

`V3.parent_version_id` is `NULL`; `V4.parent_version_id` is `V3`. No
cross-Workspace Version parent is attempted. Route B's source remains the
explicit Checkpoint anchor, and C1 remains bound to V1 after cold reopen.

The target Workspace-B Execution is created with no base/current Version
before local publication. This preserves the existing Execution base-Version
scope validator while materialization supplies the source state. Resume is
validated from C1 on the source Workspace A, where the existing Resume
contract is applicable; Route membership is attached explicitly in both cases.

## Covered invariants

```text
Fork from Version: PASS (Route A explicit Version source)
Fork from Checkpoint: PASS (Route B explicit Checkpoint source)
Source anchor persistence: PASS
Workspace-local root Version: PASS
Cross-workspace parent attempted: NO
Version parent invariant: PRESERVED
Parallel routes and separate Workspaces: PASS
Rollback + Fork history preservation: PASS
Handoff: PASS
Resume: PASS
Route lifecycle statuses: PASS
Scope/idempotency boundary: PASS
Cold reopen: PASS
Operation ownership: PASS
```

Route A's interrupted Execution remains history while Route A retains its
active status. Route B remains independently attached to Workspace B. V2,
Checkpoint C1, Handoff, Resume, and Rollback records all survive repository
reopen.

## Boundaries

```text
Protocol v1.0: UNCHANGED
Candidate/Evaluation/Selection: NOT IMPLEMENTED
Provider runtime: NOT TOUCHED
easyCode: NOT TOUCHED
D:\\bs\\seekwd: NOT TOUCHED
Version semantics: UNCHANGED
Schema migration: NONE
```
