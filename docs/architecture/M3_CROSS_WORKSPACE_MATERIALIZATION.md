# M3 Cross-Workspace Materialization Contract

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003E

## Source / Target Boundary

Cross-Workspace operations have a read-only Source and an explicitly authorized
Target. The Source is an immutable Version/Snapshot owned by its original
Workspace. The Target is another Workspace's physical tree and local durable
metadata. The source Workspace is never leased, updated, or selected as a side
effect of a target operation.

## Materialization Flow

1. Validate source Version, Snapshot, manifest, CAS, project, environment,
   generation, and migration identity.
2. Validate target Workspace identity, lease, and expected revision.
3. Read source content and materialize it into the target using existing local
   driver machinery.
4. Verify the target tree against the source manifest.
5. Publish a target-local Snapshot metadata row and target `Workspace.head`
   through the existing durable transaction boundary.

The source Snapshot ID is an input reference. The resulting target Snapshot ID
is local metadata. CAS remains globally reusable by content address; Snapshot
ownership and metadata remain local.

## Snapshot and Head Rules

`S100[W1]` may be read by W2, but W2.head can only reference a Snapshot row
published for W2. A target result with equal content therefore has distinct
local metadata and is not an ownership transfer. Version Head remains local and
cannot point at V100[W1] unless a future contract explicitly changes that rule.

## Diff

Diff reads the source manifest and compares it to the target current tree. It is
read-only: no CAS, SQLite, lease, revision, operation, event, or Head mutation.
The M2 observation/reconciliation rule applies to target identity and revision;
changes during the scan return `UNSTABLE_OBSERVATION`/Conflict.

## Restore

Restore from a foreign Workspace source is a target materialization operation,
not a source Snapshot reassignment. It must retain the existing restore
operation identity and target transaction semantics, while recording source
Version/Snapshot IDs as bounded input references.

## Rollback

The local rollback contract remains unchanged. A cross-Workspace rollback would
materialize the source into a target-local Snapshot, but the resulting head and
Version Head semantics are not currently frozen. Until an explicit successor
contract resolves that question, cross-Workspace rollback is rejected as
`CONTRACT_CONFLICT`; this slice does not implement it.

## Parallelism and Isolation

Concurrent materialization, restore, and diff operations on W1/W2/W3 are
independent when leases and revisions are valid. Same-Workspace concurrent
writers conflict deterministically. Source CAS reads never serialize unrelated
Workspace mutations.

## Failure and Recovery

Source validation, provider, filesystem, lease, revision, transaction, or
verification failure must not publish a target head. Before-commit failure
leaves old target metadata; after-commit uncertainty is reconciled by reopen.
Only a fully verified and durably published target is success. Exact retries
must return the existing operation/result or a deterministic conflict.

## Schema Proposal

Prefer the existing Execution base field or one explicitly named additive
Execution-level reference, plus existing Operation refs and local Snapshot
publication. No global Snapshot table, source-copy table, or reconciliation
entity is required by this proposal. If implementation needs more than this
minimal additive representation, stop with `SCHEMA_GAP`.

## Compatibility

M1 v0.1.0, M2 Snapshot ownership, Snapshot identity, Restore, Diff, Rollback,
Workspace Head, Version identity/parent/Head, Operation identity, and legacy
read semantics remain unchanged. No DDL or runtime implementation is included.

## Status

`PROPOSAL ONLY`.
