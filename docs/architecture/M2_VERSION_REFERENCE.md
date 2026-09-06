# M2 Version Reference / Head Model

> **INTERNAL / TEST-GATED** - M2-SLICE-012B implementation of the bounded
> M2-SLICE-012A contract. This is not a public API or release qualification.

## Snapshot Head

`WorkspaceRecord.head` is the current immutable content reference: an optional
Snapshot root digest. Snapshot publication and workspace status/diff continue
to read this field as a content head. Version creation and parent graph writes
do not change it.

## Version Head

A Version Head is a separate explicit nullable workspace reference to one
durable logical Version. It is not the Snapshot root digest, the latest Version
row, the parent of a Version, or a branch. Broader scopes remain open.

```text
Snapshot Head: Workspace -> Snapshot root digest
Version Head: Workspace.version_head_id -> Version -> Snapshot
Lineage:       Version -> parent Version
```

The model must allow the Snapshot Head and Version Head to differ. A content
head of `S2` does not prove that `V2` is selected, even when `V2 -> S2` exists.

## Version Graph

011B defines an immutable single-parent graph. A null parent is a root; a
non-null parent must already exist and share workspace, project, environment,
generation, and migration scope. Parentage is historical `based on` lineage,
not current selection. The graph does not provide a total order.

## Version Selection

Selection must name a durable Version explicitly and validate its
workspace/project/environment/generation/migration bindings. Selecting `V2`
must not mutate `V2`, its Snapshot, its parent, or `Workspace.head`. Unselected
Versions remain readable detached history.

`get_current_version(workspace_id)` returns `None` for a null reference and
otherwise returns the durable Version after validating its Snapshot, operation,
parent chain, and workspace/project/environment/generation/migration scope.
`set_version_head` is the lease/revision-guarded write path and does not infer
or select a Version during Version creation.

## Identity

The existing Version ID remains derived from project, workspace, Snapshot, and
creation-operation identity. Parent is immutable but is not part of the ID.
Human labels and a future ordinal must not alter this identity. A Version Head
reference is selection metadata, not a new Version identity input.

## Detached Versions

If a reference selects `V3`, earlier durable `V1` and `V2` remain valid
historical Versions, including when they are no longer selected. Detachment
does not authorize deletion, rewriting, or parent changes.

## Workspace Binding

The implemented bounded scope for a Version Head is workspace-local because
the current Snapshot Head, lifecycle lease, and Version bindings are
workspace-scoped. Project-wide or global references require a separate
contract and must not be inferred from the current model.

## Generation and Environment Binding

Selection cannot cross the existing 011B bounded scope. The selected Version
must match workspace, project, nullable environment identity,
`generation_id`, and `migration_id`. A future cross-generation reference needs
an explicit migration/attestation contract; it cannot be created by copying a
Version ID into a new generation.

## Transaction Model

If Version creation and head selection are coupled, validation, Version insert,
head write, operation completion, and journal updates must commit as one
SQLite transaction. If selection is separate, it still requires a durable
operation identity and an accepted lease/revision guard. In either model a
committed reference may point only to a committed, healthy Version.

No DDL or transaction implementation is part of 012A.

## Recovery Model

Before commit, a failed operation exposes neither a new Version Head nor a
completed selection. An unknown post-commit result is recovered by cold reopen:
the reference must be either the previous complete value or the new complete
value, and the selected Version must exist with valid Snapshot and graph
bindings. Exact retry returns the existing durable result; it never produces a
second selection or a dangling pointer.

## Deletion and Retention

Deletion is not implemented. A selected Version must be protected from
deletion, as are parent Versions and Snapshots already protected by the graph
contract. Cascade deletion is prohibited until a separate retention/GC
contract is accepted.

## Compatibility

M1 `v0.1.0` and old-reader semantics remain unchanged. A v0.1 migration has no
synthetic Version and no inferred Version Head. `Workspace.head` keeps its
Snapshot-root meaning for all existing readers and APIs.

## Open Decisions

The following are intentionally unresolved: Version Head existence and scope,
same-Snapshot multiple-Version policy, selection operation shape, human labels,
ordinal numbering, cross-generation references, and deletion/GC behavior.

## Production Status

`PRODUCTION_IMPLEMENTATION = INTERNAL / TEST-GATED`

M2-SLICE-012B implements the nullable additive `workspaces.version_head_id`
column, deterministic read/write validation, revision CAS, lease checks,
atomic transaction boundaries, retry/recovery, cold reopen, and legacy NULL
migration. Branch, merge, numbering, labels, deletion/GC, and cross-generation
reference behavior remain outside this slice.
