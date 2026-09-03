# ADR-M2-003: Workspace Status View

- **Status:** Proposed / internal M2 slice
- **Date:** 2026-09-01
- **Baseline:** M1 `v0.1.0`, commit `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

## Context

The bounded local workspace slices already persist logical workspace metadata,
leases, environments, immutable snapshot metadata, and operation outcomes. A
caller needs one read-only, point-in-time view without reading SQLite rows or
guessing from a physical path.

## Goal

Expose an internal, serializable status view that reports authoritative
workspace identity and revision, the verified snapshot head, lease state,
environment binding, filesystem change state, execution readiness, and the
latest/unresolved workspace operation state.

## Non-goals

This ADR does not define a public API, CLI output contract, filesystem watcher,
background index, diff, commit/version/branch model, provider state, or a new
workspace lifecycle transition.

## Status Fields

`WorkspaceStatus` contains the logical workspace/project IDs, local driver,
metadata revision, lifecycle status, optional `head_snapshot_id` and
`head_digest`, lease summary, environment binding summary, `changed`,
`change_state`, `healthy`, `execution_ready`, `recovery_required`, and an
optional latest operation summary. The physical locator is not exposed as a
status identity field.

## Source of Truth

Workspace identity, revision, lifecycle status, environment ID, and head digest
come from the durable `workspaces` row. Lease epoch/owner/expiry come from the
durable `workspace_leases` row. Snapshot identity and content come from the
durable `snapshots` row and verified CAS manifest/blobs. Operation summaries
come from the durable operation ledger.

## Derived Fields

`head_snapshot_id` is derived only after the head digest resolves to one
matching snapshot record. `changed` means the point-in-time local filesystem
tree differs from the current published snapshot (semantic B); it is `None`
with `change_state = "no_snapshot"` when no head exists. A filesystem scan is
read-only and computes file digests in memory. `lease.active` requires an owner
and `expires_at_ms > now_ms`. `healthy` is false when integrity cannot be
verified or an unresolved operation exists. `execution_ready` requires a
`ready`/`active` lifecycle state, a valid environment binding, an existing
verified head, and no unresolved operation; it does not acquire a lease.

## Revision and Head Semantics

Revision is the persisted optimistic-concurrency revision and is not a
filesystem change counter. The head is the verified `sha256:<tree-digest>`
root recorded by snapshot publication; the status view never advances either
field.

## Lease Semantics

Status is read-only and does not renew, acquire, release, or otherwise mutate a
lease. An expired or released row is reported as inactive. A stale token is not
accepted or treated as current because status reports the durable current row.

## Environment Binding

An environment ID is `bound` only when the immutable environment exists and
belongs to the workspace project. Missing or cross-project bindings are an
integrity error. No environment facts are copied into the status view.

## Changed and Unknown State

The changed value is a comparison to the current head snapshot, not a revision
comparison and not a claim about an external provider. A missing head has no
comparison baseline. An unresolved `started` or `unknown` operation is exposed
through `recovery_required` and the operation summary; it is never reported as
healthy or execution-ready.

## Recovery and Cold Reopen

Status revalidates the same metadata/CAS identity after every open. It fails
closed with the existing `INTEGRITY_ERROR`, `NOT_FOUND`, `PERMISSION_DENIED`,
or storage error codes when the head, snapshot, environment, manifest, or
filesystem cannot be verified. Reopening the repository must produce the same
authoritative fields without a write side effect.

## Compatibility

The view is additive and internal. A v0.1 repository with no M2 snapshot head
remains openable and reports `change_state = "no_snapshot"`; no legacy reader
is required to understand `WorkspaceStatus`.

## Testing

Tests cover create/no-head, published and changed trees, restore operation
summaries, lease expiry/takeover, unresolved operations, head/snapshot and
environment corruption, cold reopen, serialization, and legacy/no-snapshot
behavior. Filesystem failpoints and external watchers are out of scope.
