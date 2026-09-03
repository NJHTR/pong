# ADR-M2-001: Atomic Snapshot Publication

- **Status:** Proposed for M2 internal implementation
- **Date:** 2026-08-30
- **Baseline:** M1 `v0.1.0`, commit `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

## Context

The bounded M2 local workspace slice can already capture a canonical tree
manifest and immutable file blobs in the existing CAS. The current workspace
manager advances `WorkspaceRecord.head` in a later metadata transaction, but it
does not persist a first-class snapshot metadata record or link publication to
the operation/event ledger. A crash or commit failure therefore needs an
explicit publication contract.

M1 already fixes the repository generation, SQLite transaction, CAS identity,
redaction, Event Envelope, project sequence, operation ledger, and migration
selector semantics. This ADR adds only the snapshot publication boundary; it
does not revise those M1 contracts.

## Problem

How can Pong publish a local snapshot so that CAS content, snapshot metadata,
workspace head, and the snapshot-created event are observed consistently after
failure and cold reopen?

## Goals

1. Give each snapshot a typed `snapshot_id` distinct from its CAS tree digest.
2. Persist minimal, generation-bound snapshot metadata.
3. Publish snapshot metadata and workspace head in one SQLite transaction.
4. Link the publication to an existing durable operation and Event Envelope.
5. Ensure a failed transaction leaves the old head and no partial metadata, or
   an explicitly recoverable complete post-state after a post-commit error.
6. Preserve v0.1 source rows and old-or-new generation migration behavior.

## Non-goals

This ADR does not define restore-over-existing-workspace, diff, commits,
branches, checkpoints, version parents, incremental snapshots, provider
reconciliation, public CLI/SDK, remote drivers, or a new event/projection
contract.

## Alternatives

1. **Head-only digest:** rejected because it cannot explain provenance,
   operation linkage, or publication state after restart.
2. **Copy the entire workspace into metadata:** rejected because immutable bytes
   belong in CAS and copying bypasses content identity.
3. **Separate metadata and head transactions:** rejected because a crash can
   expose a head without an explainable snapshot or metadata without a visible
   head.
4. **New metadata database:** rejected because M1 binds metadata, events,
   generation identity, and migration to the active repository generation.

## Snapshot Identity

`Snapshot` exposes two distinct values:

- `snapshot_id`: a typed, stable identifier (`snp-<tree-digest>`) used by
  metadata, operations, and events;
- `root_digest`: the CAS digest of the canonical `workspace/tree/v1` manifest.

The snapshot ID is not a commit, branch, checkpoint, workspace revision,
operation ID, or event ID. `workspace revision` remains the optimistic
concurrency version of the workspace row. The initial bounded implementation
derives the typed ID from the verified tree digest; a future ADR may choose a
different namespace without changing the CAS bytes.

## Snapshot Metadata

The additive `snapshots` table records the snapshot ID, root digest,
workspace/project/environment identity, manifest and redaction profile
versions, file count, total bytes, creation time, operation/event IDs, and the
active generation/migration identity. CAS remains authoritative for manifest
and blob bytes; metadata disagreement is an integrity/recovery condition.

## Workspace Head

`WorkspaceRecord.head` stores the protocol text `sha256:<root_digest>`, not the
typed snapshot ID. The snapshot metadata row is the explanation for that head.
The head advances only when the caller presents the current lease and expected
workspace revision and the referenced CAS manifest has already been verified
by the local driver.

## Atomic Publication

The local driver publishes file blobs and the canonical tree manifest before
the metadata transaction. The metadata transaction then:

1. validates workspace/project/environment, lease epoch/expiry, revision,
   snapshot identity, redaction profile, and generation identity;
2. inserts or verifies the snapshot metadata row;
3. appends the `snapshot.created` Event Envelope with operation, workspace,
   causation, and correlation links;
4. updates the workspace head and revision; and
5. commits with the existing SQLite `FULL` durability boundary.

Before commit, any failure rolls back metadata, event, head, and revision. A
verified CAS object may remain unreachable and is not treated as a published
snapshot. An error after SQLite commit is outcome-unknown; cold reopen is the
oracle and must observe the complete post-state.

## Event/Operation Linkage

The caller starts one existing operation before publication. The snapshot
metadata row and `snapshot.created` envelope use that operation ID. The event
uses the operation-start event as causation and the operation/request identity
as correlation. The operation is finished after successful publication; if the
process stops between publication and finish, the existing operation recovery
path reports an unfinished operation without deleting the published snapshot.

The M1 Event Envelope columns, project sequence allocation, generation binding,
and projection behavior are unchanged.

## Crash Behavior

- CAS failure: no metadata/head publication occurs.
- Metadata insert or head-update failure before SQLite commit: old workspace
  head/revision and no snapshot metadata/event are visible after reopen.
- SQLite post-commit error: metadata, event, and new head are all visible after
  reopen; retry is idempotent or requires explicit reconciliation.
- Process termination at any boundary follows the same pre-state/post-state
  oracle; no partial committed head is valid.

## Recovery

Cold reopen verifies the snapshot metadata row, manifest digest/domain,
workspace/project identity, generation identity, and head relation. A row or
head without its counterpart is an integrity/recovery condition, not an
automatic repair. Verified but unreachable CAS objects remain eligible for a
later explicit retention/quarantine policy.

## Migration From v0.1.0

The `snapshots` table is additive. Opening a legacy v0.1 metadata file through
the ordinary M2-compatible path may initialize additive tables without changing
legacy rows; migration's read-only backup entrance remains schema-preserving.
A target generation initializes and validates the table before selector
publication. No legacy reader is assumed to understand snapshot metadata.

## Compatibility

Existing event, operation, projection, generation, redaction, and migration
fixtures remain authoritative. Unknown additive event payload fields remain
opaque. No public API or released repository-format guarantee is created by
this internal ADR.

## Rollback

There is no in-place rollback of a committed snapshot publication. Before
commit, SQLite rollback preserves the old head. After commit, recovery keeps
the complete new publication; reverting a workspace requires a later explicit
restore operation and must not rewrite historical events or CAS objects.

## Testing

The first implementation must add integration coverage for successful CAS and
metadata publication, metadata/head pre-commit failures, post-commit cold
reopen, snapshot identity separation, operation/event linkage, and additive
v0.1 migration. The complete M1 regression suite must remain passing. Fault
injection evidence is synthetic and must not be represented as native host
evidence.
