# M3 Cross-Workspace Rollback Schema Contract

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003H

## Existing Schema

The current `rollback_records` table already contains:

```text
rollback_id, task_id, execution_id, workspace_id,
target_kind, target_reference, target_version_id,
actor_agent_id, request_id, request_digest, status,
result_version_id, generation_id, migration_id,
created_at, updated_at
```

`target_reference` is the existing typed-by-`target_kind` reference. For a
Checkpoint target it is the Checkpoint ID; no duplicate `checkpoint_id` column
is proposed. `target_version_id`, `result_version_id`, `status`, and the timing
fields already exist.

## Current Gap

The current row cannot distinguish the immutable source state from the target
Workspace result. It cannot durably answer which source Snapshot was read, what
target Workspace Head existed before materialization, or which target-local
Snapshot became the result. Current completion also assumes the target Version
can be assigned to the target Version Head, which is invalid for a foreign
Workspace source.

## Source State

The source is an immutable `source_version_id` and its immutable
`source_snapshot_id`. The source Version and Snapshot remain owned by their
original Workspace. The source fields grant no lease or write authority.

## Target State

The target is the request `workspace_id`, its previous local content head, and
the resulting local Snapshot head. The target Snapshot must be owned by the
target Workspace. CAS bytes may be shared; Snapshot metadata may not be
reassigned.

## Rollback Record

The additive proposal is:

| Field | Existing or proposal | Meaning |
| --- | --- | --- |
| `rollback_id` | existing | request identity |
| `task_id`, `execution_id` | existing | authorized scope |
| `workspace_id` | existing | target Workspace |
| `target_kind` | existing | Version, Checkpoint, or existing target mode |
| `target_reference` | existing | Checkpoint ID when `target_kind=checkpoint` |
| `target_version_id` | existing | requested historical Version target |
| `source_version_id` | **new nullable** | immutable source Version read by the operation |
| `source_snapshot_id` | **new nullable** | immutable source Snapshot read by the operation |
| `previous_workspace_head` | **new nullable** | target head before materialization |
| `result_workspace_head` | **new nullable** | target-local Snapshot root after materialization |
| `previous_version_head` | **new nullable** | target Version Head before rollback |
| `result_version_head` | **new nullable** | target Version Head after rollback |
| `result_version_id` | existing | remains `NULL` for rollback |
| `status` | existing | recovery state |
| `request_digest`, generation/migration, timestamps | existing | idempotency/integrity/audit |

For a new cross-Workspace rollback, `source_version_id` and
`target_version_id` identify the requested source/target relationship, while
the source Snapshot and target-local result are explicit. For legacy local rows,
the new columns remain NULL and existing fields retain their old meaning.

## Workspace Head

`result_workspace_head` must be the root digest of a Snapshot owned by the
target Workspace. A foreign source Snapshot ID is never written into
`Workspace.head`. `previous_workspace_head` and `result_workspace_head` make
old/new recovery and cold reopen explainable without parsing JSON.

## Version Head

Three candidates were considered:

- **A: unchanged:** leave target Version Head unchanged and set
  `result_version_head = previous_version_head`.
- **B: clear:** set target Version Head to NULL.
- **C: create a target Version:** prohibited because rollback creates no Version.

**Recommendation: A.** M2 already defines `Workspace.head` and Version Head as
independent references. A cross-Workspace rollback changes the target's current
physical/local Snapshot but does not silently change its logical Version
selection. This durable divergence is valid and explicit. A later Resume or
Version publication can create/select a target-local Version. A foreign Version
is never assigned to `result_version_head`.

## Snapshot and Version Semantics

Rollback may publish a target-local Snapshot without creating a Version. Snapshot
publication and Version creation are separate operations/results. Historical
Versions, identities, parent edges, and source Snapshot metadata remain
unchanged. `result_version_id` is always NULL for this rollback result.

## CAS

CAS manifests and blobs remain immutable and globally reusable by digest. The
target Snapshot metadata and Workspace Head remain target-local.

## Atomicity

The physical materialization boundary remains prepare -> materialize -> verify
-> publish metadata -> reconcile on reopen. The SQLite metadata transaction
publishes the typed rollback result and target Workspace Head together. SQLite
and the physical filesystem are not treated as one ACID transaction.

## Recovery

Existing `prepared` and `completed` states remain valid. The contract permits
the existing recovery vocabulary `prepared`, `materialized`, `published`,
`completed`, `failed`, and `unknown` for future implementation checkpoints;
legacy rows are not rewritten. Completion requires a verified target-local
Snapshot and matching `result_workspace_head`. An uncertain physical or SQLite
boundary is `unknown` until reopen reconciliation.

## Idempotency

The existing `(task_id, request_id)` and `request_digest` identify exact retry.
The durable row must include source, target, expected previous head, and result
head so a retry can return the same result without a second Snapshot publication
or revision increment. Changed source, target, Workspace, or expected revision
is deterministic conflict.

## Migration

Future migration is additive only:

```sql
ALTER TABLE rollback_records ADD COLUMN source_version_id TEXT;
ALTER TABLE rollback_records ADD COLUMN source_snapshot_id TEXT;
ALTER TABLE rollback_records ADD COLUMN previous_workspace_head TEXT;
ALTER TABLE rollback_records ADD COLUMN result_workspace_head TEXT;
ALTER TABLE rollback_records ADD COLUMN previous_version_head TEXT;
ALTER TABLE rollback_records ADD COLUMN result_version_head TEXT;
```

The SQL above is a proposal, not executed DDL. Existing v0.1 rows receive
NULLs. No synthetic source Snapshot, target Snapshot, Version Head, or
cross-Workspace relation is backfilled.

## Compatibility

M1 v0.1.0, ADR-0015, ADR-0016, M2 Snapshot ownership, Restore, Workspace Head,
Version identity/parent/Head, Operation identity, and legacy read semantics are
unchanged. This proposal does not promote Version or Snapshot ownership to
Project scope.

## JSON Hack Prohibition

The six new values must be typed durable columns. Storing them in `refs.value`,
operation result JSON, or an opaque metadata blob would make indexed queries,
integrity validation, additive migration, cold-reopen recovery, idempotency,
and auditability dependent on ad hoc parsing. The existing result JSON remains
an output detail, not the authoritative rollback record.

## Alternatives

- Reuse `target_reference` JSON: rejected; it loses typed query and integrity
  guarantees.
- Add a separate RollbackResult entity: unnecessary for the six additive values
  and would expand core schema.
- Change Version Head to permit foreign Versions: rejected as contract conflict.
- Create a Version for every target Snapshot: rejected by frozen rollback
  semantics.

## Open Decisions

Exact migration version, whether `source_version_id` is mandatory for all new
rows, operation/event linkage for target Snapshot publication, authorization for
cross-Workspace rollback, and public API exposure remain open for 003G.

## Status

`PROPOSAL ONLY`. No DDL, migration, production code, or runtime behavior is
implemented in this slice.
