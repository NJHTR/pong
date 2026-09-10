# M3 Local Materialized State / Version Decoupling

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003F

## Purpose

This proposal closes the source/target representation needed after
M3-SLICE-003D and 003E. It does not change M2 ownership, Snapshot identity,
Version identity, Version parentage, or the meaning of `Workspace.head`.

## State Vocabulary

### Source

An immutable historical Version, its Workspace-local Snapshot metadata, and the
verified manifest/blob content in CAS. A source is read-only to the target
Execution.

### Working State

The target Workspace physical tree while an Execution is editing it. Working
State is not automatically a Version and is not a durable Version Head.

### Target Snapshot

An immutable Snapshot metadata publication owned by the target Workspace. It is
created after a target materialization or snapshot operation is verified. It
may reuse source CAS objects, but its metadata and publication operation are
target-local.

### Version

An immutable logical node owned by one Workspace. A Target Snapshot does not
become a Version unless a later explicit Version publication operation creates
one under the existing M2 contract.

### Workspace Head

The content head of the target Workspace. It points only to the root digest of a
Snapshot metadata row owned by that Workspace. After W2 materializes V100[W1],
W2.head points to the newly published W2-local Snapshot, never S100[W1].

### Version Head

The target Workspace's explicit logical Version selection. It is independent
from Workspace Head and remains either the prior valid W2 Version or `NULL`
until a later explicit local Version selection. It never points at a foreign
Workspace Version under this contract.

## Core Decision

`SOURCE -> TARGET WORKING STATE -> TARGET SNAPSHOT` is the only accepted
cross-Workspace materialization path. Source metadata is not reassigned and
source Snapshot identity is not copied into target metadata. CAS content is
shared by immutable digest where possible.

This means a target Snapshot may exist without a corresponding Version. That is
intentional and already compatible with M2's independent nullable
`Workspace.head` and `version_head_id` references. The state

```text
W2.head = S200[W2]
W2.version_head_id = V200[W2] or NULL
```

is valid even when S200 is not the Snapshot referenced by V200. This is not a
phantom success or a broken Version; it identifies current working content and
the separately selected logical Version.

## Restore

Cross-Workspace restore reads a verified source Version/Snapshot, materializes
the source content into the target Workspace, verifies the target tree, and
publishes a target-local Snapshot. It updates W2.head to that local Snapshot
under the existing lease/revision and operation boundaries. It does not change
the source Workspace, source Version, source Snapshot, or W2 Version Head.

The existing M2 restore operation remains source-Snapshot based and local to
its existing destination contract. A future cross-Workspace restore adapter
must record source Version/Snapshot IDs as input references while retaining
target-local output metadata.

## Rollback

Same-Workspace rollback keeps the frozen M3-SLICE-002C/002D behavior: materialize
the local target Version, update both local content and logical heads, preserve
history, and create no Version.

Cross-Workspace rollback uses the same source/target physical flow as restore,
but its target result is deliberately different from a local Version rollback:

1. Resolve the foreign source Version as read-only.
2. Materialize and verify its content in the authorized target Workspace.
3. Publish a target-local result Snapshot and set target Workspace Head to it.
4. Leave target Version Head unchanged (or `NULL`); never assign a foreign
   Version to it.
5. Keep `result_version_id = NULL`; no Version is created by rollback.

This is a scoped extension for a future cross-Workspace rollback operation. It
does not reinterpret the existing local rollback contract and must fail closed
until that operation is explicitly implemented.

## Diff

Two references are distinct:

- normal Workspace diff compares current target tree with the target's local
  Workspace Head Snapshot;
- an explicit source diff compares current target tree with the verified
  source Version/Snapshot manifest.

Both are read-only observations. Source diff never changes target heads,
Version Head, lease, revision, Operation, event, or CAS. M2's
`UNSTABLE_OBSERVATION` reconciliation rule applies to target identity and
revision during a scan.

## Resume

Resume carries the immutable source Version reference into a new Execution. A
target Snapshot alone is not a Version and cannot be silently placed in
`base_version_id`. A later explicit local Snapshot plus Version publication may
create a new W2 Version with valid same-Workspace parentage. Resume does not
rewrite the failed Execution or source Version.

## Handoff

Handoff carries only the opaque source Version ID and bounded integrity facts.
The receiving Execution independently authorizes its target Workspace and
acquires that Workspace lease. Handoff never transfers source ownership or a
lease.

## Isolation

W1 source metadata and heads remain unchanged. W2 owns its physical tree,
target Snapshot, Workspace Head, Version Head, revision, lease, Operations, and
recovery state. Parallel targets can materialize the same source concurrently.

## Failure and Recovery

Missing/corrupt source, missing CAS, incompatible project/environment/
generation/migration, lease/revision conflict, filesystem failure, or metadata
failure fails closed. Before publication, target metadata remains old. After
an uncertain boundary, reopen reconciles the existing operation and target
Snapshot/head; success is reported only for verified local materialization plus
durable publication. No foreign head or phantom Version is published.

## Idempotency

The existing operation/request identity covers source ID, target Workspace,
action, and expected revision. Exact retry returns the same target-local result
after verification. A changed source, target, action, or request digest is a
deterministic conflict.

## Compatibility

No M1, ADR-0016, M2 Snapshot ownership, Snapshot identity, Restore, Diff,
Workspace Head, Version identity, Version parent, Version Head, Operation
identity, or legacy reader meaning changes. No schema or DDL is introduced by
this contract.

## Open Decisions

The concrete operation name for cross-Workspace materialization; whether its
target Snapshot publication uses a dedicated result reference; cross-Workspace
rollback authorization; checkpoint reuse; and public API exposure remain open.

## Status

`PROPOSAL ONLY`. Runtime implementation is deferred to the approved successor
slice.
