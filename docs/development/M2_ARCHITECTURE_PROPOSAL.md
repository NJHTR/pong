# M2 Architecture Proposal

**Status:** design proposal; the atomic snapshot-publication portion is
implemented internally by M2-SLICE-001.
**Baseline:** M1 `v0.1.0` / `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

## Goal

Extend the durable core with a bounded local workspace/snapshot vertical slice:
capture a verified workspace tree, publish it through CAS and metadata, and
materialize it safely after restart without weakening M1 guarantees.

## Scope

Local filesystem workspaces, epoch leases, environment binding, immutable tree
snapshots, snapshot metadata, controlled restore to a new destination, and the
tests/evidence required by `M2_WORKSPACE_SNAPSHOT_GATE.md`.

## Non-goals

No commit/branch/version DAG, semantic merge, external-effect rollback,
provider reconciliation, incremental snapshot optimization, remote driver,
public CLI/SDK, server, or framework adapter.

## Existing M1 Foundations

M2 reuses `Repository` generation selection and identity checks, `MetadataStore`
transactions and event envelopes, the operation ledger, redaction policy,
typed CAS domains, canonical serialization, workspace leases/revisions, and
M1 recovery/failpoint behavior. No M2 design may bypass these boundaries by
opening SQLite directly or by treating a filesystem path as identity.

## Workspace Model

`Workspace` is a logical project-scoped identity bound to a driver and a
materialization locator. The existing local driver remains the only
implementation. A mutation requires the current lease token and expected
workspace revision. The workspace head is a reference to a verified snapshot
identity; it is advanced only after CAS publication and the metadata
transaction's durability boundary.

Workspace lifecycle status remains a guarded state transition, not an inferred
filesystem lifecycle. M2-SLICE-003 additionally exposes a read-only
`WorkspaceStatus` observation: it preserves the durable lifecycle state while
deriving lease activity, head validity, filesystem change state, execution
readiness, and unresolved-operation state. It does not add a watcher or mutate
the workspace.

## Operation Model

Snapshot and restore are operations carried by the existing immutable operation
envelope. A snapshot operation records workspace/environment identity, input
preconditions, resulting snapshot reference, and outcome. A restore operation
records source snapshot, destination/precondition, verification result, and
whether publication was durable or outcome-unknown. Operation IDs, request
idempotency, terminal lifecycle, redaction, and operation events remain exactly
the M1/M3 contracts.

## Snapshot Model

A snapshot is a factual immutable tree state, not a commit or checkpoint. Its
CAS identity is the canonical digest of a `workspace/tree/v1` manifest;
`snapshot_id` is the typed `snp-<tree-digest>` metadata identity. File bytes
are immutable `workspace/blob/v1` CAS objects. The manifest includes the
workspace/project and redaction-profile identities. M2-SLICE-001 persists
snapshot/project/workspace/environment, manifest/redaction, count, operation,
event, generation, and migration facts in the additive `snapshots` table.

Metadata is an index/attestation; CAS bytes remain authoritative for content.
Any disagreement is an integrity/recovery condition, never a reason to rewrite
the manifest digest.

## Version Model

M2 does not add commits, branches, or version parents. `WorkspaceRecord.head`
may continue to point to a snapshot digest as an internal bounded reference,
but this must not be documented as a commit or public version ID. Phase 4 will
define immutable version nodes and parent/ref semantics.

## Restore Model

The first restore operation materializes a verified snapshot into a destination
that does not already exist. It builds a sibling temporary tree, verifies every
blob and path, syncs files/directories, and atomically publishes the directory.
An existing destination is a conflict. A post-rename parent-sync failure is
outcome-unknown and must retain the destination for explicit reconciliation;
the implementation must not delete user data or claim a rollback it did not
perform.

Restoring over an existing workspace, safety snapshots, merge/overwrite
policies, and external resource restoration are later decisions requiring
separate contract work.

## Event Flow

```text
request
  -> validate identity, lease, environment, and preconditions
  -> start operation + durable intent/event
  -> read workspace and publish file blobs
  -> publish canonical tree manifest
  -> transactionally record snapshot metadata + snapshot-created event + workspace head
  -> finish the operation after publication or reconcile an outcome-unknown retry
  -> acknowledge durable or explicitly unknown outcome
```

M2-SLICE-001 specifies this transaction boundary in
`ADR-M2-001-snapshot-publication.md`. An unreachable but verified CAS object is
acceptable; a workspace head pointing at an unverified or missing manifest is
not.

## State Flow

```text
created --lease--> capturing --CAS+metadata--> ready(head=snapshot)
ready --continue--> active
ready --restore(new destination)--> materialized
capturing --fault/crash--> reconciling or unknown
```

Only transitions supported by the current metadata validation are in the first
slice. `reconciling` is a decision state, not an automatic success state.

## Persistence

Use the active generation's `MetadataStore` and existing CAS. The
`snapshots` metadata is additive and generation-bound, initialized in the same
manner as existing M2 tables. A future schema addition requires an
`ADR-M2-*` and raw v0.1 source-preservation fixture updates. Do not introduce
a second metadata database or a path-derived identity.

## Recovery

Cold reopen must distinguish: no published snapshot metadata/head; fully
published snapshot; verified but unreachable CAS; and restore publication with
unknown parent-sync outcome. Recovery may quarantine or retain objects and
temporary directories according to an explicit policy, but may not infer a
successful provider mutation from an absent terminal event.

## API / SDK Boundary

For M2, the Rust APIs remain internal/test-gated. The stable boundary is a
repository-owned Rust domain API that accepts typed IDs, lease tokens,
preconditions, and typed errors. SQLite handles, CAS filesystem paths, and
driver-specific locators stay internal. CLI, Python SDK, Node.js SDK, and
framework adapters are not implemented in this phase. A public API proposal
must wait until the snapshot/restore semantics and error model have tests and,
if it changes a contract, an ADR.

## Migration From v0.1.0

Opening a legacy repository must remain read-only at the source and must not
silently add M2 tables. A target generation may initialize additive snapshot
metadata after copying and verifying the source. Selector publication remains
the only visibility commit point; old-or-new visibility and generation identity
checks remain unchanged.

## Testing Strategy

Keep the complete M1 suite as a regression gate. Add focused unit/integration
tests before implementation for snapshot metadata atomicity, event linkage,
path and secret rejection, diff determinism, restore destination conflicts,
crash/reopen outcomes, and orphan handling. Add only the property corpus
defined by the M2 test plan; do not reinterpret M1 corpus policy.

## Performance Considerations

Measure repository open, snapshot traversal, CAS blob publication, manifest
publication, metadata/head update, and restore materialization at documented
small/medium/large scales. Measurements are evidence, not a new capacity
budget. ADR-0015 remains the M1 owner decision and is not changed here.

## Risks

- Treating a snapshot digest as a commit would conflate factual state with
  version intent.
- Updating metadata before all CAS references are durable could publish a
  dangling head.
- Restoring in place could destroy user data on a partial failure.
- A new table initialized on the legacy source could break old-reader and
  migration guarantees.
- Path, secret, symlink, and filesystem semantics differ across platforms.
- A post-rename sync failure cannot be classified as a clean pre-state.

## Open Decisions

1. **Resolved for M2-SLICE-001:** one additive row per immutable snapshot
   identity, with the columns and generation/migration binding defined by
   `ADR-M2-001`.
2. **Resolved for M2-SLICE-001:** `snapshot.created` uses the existing Event
   Envelope contract with a typed payload; no envelope/projection schema change
   was made.
3. **Resolved for M2-SLICE-004:** the internal snapshot-to-snapshot output is
   path status plus old/new digest, size, and type metadata; current-workspace
   diff and semantic conflict output remain separate decisions.
4. Reconciliation/quarantine policy for unreachable CAS and abandoned restore
   directories.
5. Clock source and timestamp semantics for public workspace operations.
6. The point at which the internal Rust API is frozen for a future SDK.

These decisions must be resolved in an ADR before they alter M1 contracts or
the M2 schema.
