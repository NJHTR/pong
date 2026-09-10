# M3 Source / Target State Contract

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003F

## State Separation

Cross-Workspace work has four separate objects:

| Object | Scope | Mutable? | Role |
| --- | --- | --- | --- |
| Source Version/Snapshot | source Workspace | No | historical read-only source |
| Working State | target Workspace tree | Yes | current editing state |
| Target Snapshot | target Workspace | No | verified local content publication |
| Version Head | target Workspace | selection only | explicit local Version choice |

The source is never assigned as target metadata. Working State is not inferred
to be a Version. Target Snapshot publication and Version publication are two
different operations.

## Materialization Contract

```text
source Version
  -> verify source Snapshot/CAS
  -> materialize target tree
  -> verify target tree
  -> publish target-local Snapshot
  -> set target Workspace.head
```

The source Snapshot metadata remains owned by its source Workspace. The target
Snapshot metadata is owned by the target Workspace. CAS objects can be shared
by digest. The target operation must use the existing transaction, lease,
revision, and recovery boundaries.

## Head Decoupling

M2 already defines `Workspace.head` and `Workspace.version_head_id` as
independent nullable references. Therefore this is a valid durable state:

```text
Workspace W2:
  head = S200[W2]
  version_head_id = V200[W2] or NULL
```

`S200` need not be the Snapshot referenced by `V200`. This is an intentional
working-content state, not a dangling Version Head. A Version Head, when
present, must still resolve to a healthy Version owned by W2.

Cross-Workspace operations may not set `version_head_id = V100[W1]`.

## Restore and Rollback

Restore and cross-Workspace rollback both use immutable source -> target-local
materialization -> target-local Snapshot publication. Restore is a content
replacement operation. Same-Workspace rollback retains the frozen local
rollback semantics, including local Version Head update and no new Version.

For the future cross-Workspace rollback variant, the result Snapshot is local,
`Workspace.head` becomes that local digest, `Version Head` remains unchanged or
null, and `result_version_id` remains null. A foreign Version is recorded as
the source/target reference, never as a target Version Head.

## Diff

An explicit source diff compares target Working State to the source manifest.
Normal Workspace diff compares target Working State to target Workspace Head.
Both are read-only and use M2 observation/reconciliation checks. Neither
publishes a Snapshot or changes revision/lease/head.

## Resume and Handoff

Resume carries the immutable source Version ID into a new Execution. A target
Snapshot is not a Version and cannot become `base_version_id` without a later
explicit Version publication. Handoff carries the same opaque source ID and
requires independent target lease authorization.

## Ownership and Graph

Version ownership and `parent_version_id` remain Workspace-local. A target
Version created later may use a valid local parent or null. The source
reference is not a graph edge, and no cross-Workspace Merge/Branch/Rebase is
implied.

## Parallelism

Multiple targets may materialize the same source simultaneously. Their target
Snapshots, Workspace Heads, Version Heads, leases, revisions, Operations, and
physical trees remain independent. A same-Workspace concurrent writer is a
deterministic lease/revision conflict.

## Failure Boundary

Source integrity, CAS, compatibility, target lease/revision, materialization,
verification, and persistence failures produce old state, new complete state,
or deterministic conflict/unknown. Completed metadata without a verified local
tree is forbidden. Reopen reconciles uncertain operations before retry.

## Schema Proposal

No new Snapshot ownership table, global Snapshot entity, or SourceReference
entity is required by this state model. Prefer the existing Execution base
reference plus existing Operation refs and target-local Snapshot publication.
If the accepted retry/recovery contract later requires more than this minimal
additive representation, stop with `SCHEMA_GAP`.

## Compatibility

M1 v0.1.0 and all M2/M3 closed semantics remain unchanged. This document is a
proposal only and performs no migration or DDL.

## Status

`PROPOSAL ONLY`.
