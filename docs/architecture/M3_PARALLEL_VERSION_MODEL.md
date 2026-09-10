# M3 Parallel Version Model

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003C

## Model

Versions remain immutable logical nodes owned by one Workspace. An Execution
may hold an immutable, read-only Base Reference to a Version owned by another
Workspace when the reference passes project, environment, generation, and
migration validation.

```text
                 V100 [W1]
                /    |    \
               /     |     \
 BaseRef(E1) BaseRef(E2) BaseRef(E3)
             |          |          |
            W1         W2         W3
             |          |          |
          V101[W1]  V201[W2]  V301[W3]
```

## Separation of Concepts

| Concept | Authority | Meaning |
| --- | --- | --- |
| Version ownership | `Version.workspace_id` | owning Workspace of immutable logical state |
| Version content | `Version.snapshot_id` | immutable Snapshot/CAS bytes |
| Base Reference | future Execution-level additive attachment | read-only starting point |
| Execution Workspace | explicit Execution attachment | physical mutation destination |
| Workspace Head | `Workspace.head` | current Snapshot digest per Workspace |
| Version Head | existing explicit Workspace selection | current logical selection per Workspace |
| Version Parent | `parent_version_id` | same-Workspace immutable lineage only |

## Safety Rules

Base Reference validation is fail-closed for missing, corrupt, unhealthy,
wrong-project, wrong-environment, wrong-generation, or wrong-migration target.
The reference cannot mutate its source Version, source Workspace, Snapshot,
Version Head, lease, or revision. Snapshot/CAS sharing is read-only content
sharing; metadata remains isolated.

## Graph Rule

Base Reference is deliberately not a Version Graph edge. The current parent
validator therefore remains unchanged. A W2 result can be based on V100[W1]
without claiming `V201.parent_version_id = V100`; parentage must be null or a
valid W2 parent until a future cross-Workspace lineage contract exists.

## Operations

Materialization, diff, restore, rollback, and resume use the existing operation
and recovery boundaries. A read-only Base Reference does not create a new
Operation or event by itself. A mutation in W2 remains attributed to W2's
Execution and existing Workspace lease/revision scope.

## Parallel Isolation

W1, W2, and W3 have independent leases, revisions, Snapshot Heads, Version
Heads, Operations, Checkpoints, Rollback records, and physical trees. Shared
Task or base Version is not a conflict. Shared writable Workspace remains
disallowed by M3-SLICE-003A.

## Failure and Recovery

Concurrent materialization or diff of S100 may proceed independently. A
mutation failure yields old-or-new state or deterministic unknown/conflict.
Cold reopen preserves each Execution's Base Reference and never infers a
completed Version from provider output or completion order.

## Compatibility

No M1/M2 meaning changes are proposed. Existing same-Workspace `base_version_id`
bindings remain valid. Legacy v0.1 repositories remain readable and receive no
synthetic cross-Workspace relation.

## Migration Proposal

If accepted, implementation must use one minimal additive representation for
the Execution-level Base Reference and preserve all existing Version rows,
parent edges, Workspace Head, Version Head, and Operation identities. The
representation, migration/backfill policy, and old-reader behavior require a
future implementation decision. No DDL is authorized here.

## Open Decisions

Reference storage and retention; Checkpoint scope/reuse; cross-Workspace
lineage; Version Head selection; cross-project policy; and future Merge,
Branch, and Rebase.
