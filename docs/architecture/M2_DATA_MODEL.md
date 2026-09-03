# M2 Data Model

**Status:** internal model; publication, durable restore, and workspace status portions implemented by M2-SLICE-001/002/003.
**Baseline:** M1 `v0.1.0` / `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

This model describes relationships that already exist or are required by the
M2 gate. It does not create commit, branch, checkpoint, or public SDK entities.

## Aggregate View

```text
Project
  |
  +-- Workspace --(write lease)--> Agent identity
  |       |
  |       +-- Environment (same-project immutable facts)
  |       |
  |       +-- head --> Snapshot metadata --> Tree Manifest (CAS)
  |                                      \\--> File Blobs (CAS)
  |
  +-- Operation --(events)--> Event Envelope / project stream
          |
          +-- workspace_id, environment_id, snapshot refs
```

## Workspace

The current `WorkspaceRecord` contains:

- logical `workspace_id` and `project_id`;
- driver and canonical locator metadata;
- optional branch reference (reserved for later versioning);
- optional head digest;
- optional environment ID;
- guarded lifecycle status;
- monotonic revision and timestamps.

The physical path is never the identity. A workspace mutation is valid only
with a matching lease token, epoch, expiry, and expected revision. The current
local driver materializes regular files/directories outside `.pong` and rejects
unsafe paths, links/reparse points, and configured secrets.

## Provider-Neutral Lifecycle Contract

M2-SLICE-007 adds an internal contract without changing the durable schema.
`WorkspaceIdentity` is `(workspace_id, project_id, provider)`, while
`WorkspaceProviderContext` contains only identity, one of the existing
`created`/`preparing`/`ready`/`active`/`paused`/`reconciling`/`archived` states,
the lifecycle revision, and four capabilities. It intentionally contains no
filesystem path, NTFS/ext4/ACL/inode fact, Git branch, remote URI, provider
handle, or provider-specific metadata.

`Open` acquires a logical handle and is idempotent without durable status
mutation. `Close` changes a non-archived logical workspace to `archived`; it
does not imply physical deletion. Recovery is expressed by
`BeginRecovery -> reconciling -> CompleteRecovery -> ready`. Invalid actions
fail through the existing `CONFLICT` or `RECOVERY_REQUIRED` errors.

The capability set is exactly `snapshot`, `restore`, `diff`, and `status`.
Status and diff remain read-only and do not receive synthetic operations or
events. Provider actions cannot allocate project sequences, mutate generation
identity, alter projection state, or reorder events. Provider success with a
failed core persistence is unconfirmed, and core persistence with provider
failure is recorded as failure/recovery rather than success; no distributed
transaction protocol is inferred. Retries use the existing operation/request
identity and must converge or fail closed deterministically.

The current production adapter is still only `local`. The contract tests use
an explicit synthetic `TEST_DOUBLE`; provider registry, plugins, non-local
drivers, and public API serialization are not modeled.

## Durable Lifecycle Integration

M2-SLICE-008 connects the provider-neutral transition decision to the existing
lease/revision-guarded `WorkspaceUpdate` transaction. A state-changing action
persists the new existing-state value and increments revision exactly once;
`Open` is a read-like no-op. A retry may return the already durable target when
the caller presents the same transition against the immediately preceding
revision, while older or mismatched callers receive the existing conflict.
Lease and project identity remain validated by the existing metadata path.

This integration deliberately emits no generic lifecycle operation or event:
there is no accepted lifecycle event contract in M2, and status/diff remain
read-only. Snapshot and restore continue to use their existing operation/event
relations. Before-commit failures roll back; after-commit uncertainty is
reconciled by rereading durable state after reopen. Provider failure is not
converted into a core mutation or success.

## Workspace Status

`WorkspaceStatus` is a read-only derived view over the durable workspace,
lease, environment, snapshot, CAS, and operation records. Authoritative fields
include workspace/project IDs, lifecycle status, revision, head digest, lease
epoch/owner/expiry, and environment ID. Derived fields include the typed
`head_snapshot_id`, lease activity at the query time, filesystem accessibility,
`changed`, `change_state`, `healthy`, `execution_ready`,
`recovery_required`, and a compact latest-operation summary.

`changed` has one meaning in this slice: the current local regular-file tree
differs from the current published head snapshot. It is `null` with
`change_state = "no_snapshot"` when no head exists. Status computes the tree
in memory and never writes CAS, advances a revision, or changes a lease. A
missing or inconsistent head, snapshot, event, generation, environment, or
CAS blob fails closed.

## Environment

An `EnvironmentRecord` is project-scoped, immutable by ID, and contains a
canonical fingerprint plus redacted facts. A workspace may bind to one
environment only when the project matches. The current allowlisted facts are
safe descriptive inputs, not a reproducibility or container identity claim.

## Snapshot

The bounded internal `Snapshot` value is:

```text
Snapshot {
  snapshot_id,          // snp-<tree-digest>, durable metadata identity
  digest: Digest,       // canonical workspace/tree/v1 manifest CAS identity
  workspace_id,
  project_id,
  file_count,
  total_bytes,
}
```

The manifest is canonical, sorted, and includes workspace/project and
redaction-profile identity. Each file entry points to an immutable blob digest.
The digest is content identity; `snapshot_id` is its typed metadata identity.
Neither is a workspace revision, operation ID, event ID, commit, or branch.

## Snapshot Metadata

M2-SLICE-001 stores an additive durable index/attestation with:

```text
snapshot_id
root_digest
workspace_id
project_id
environment_id (nullable)
manifest_version
redaction_profile_id
redaction_profile_version
created_at
file_count
total_bytes
operation_id
event_id
generation_id
migration_id
```

The row is in the additive `snapshots` table. It is inserted with the
`snapshot.created` envelope and workspace head/revision in one SQLite
transaction. The record is an index/attestation and never overrides CAS
verification.

## Operation

The internal M3 ledger already provides operation identity, request idempotency,
project/agent/session fields, optional workspace/environment/parent links,
typed refs, terminal lifecycle, recording quality, and an event stream. M2
should reference snapshots from this envelope rather than add a parallel
operation table. Snapshot and restore operations must be distinguishable by
action/type and retain causation/correlation fields from the existing event
contract.

## Event

Events remain append-only facts with stream sequence and project sequence. A
snapshot event may reference a snapshot digest in its payload or typed ref;
the event must be redacted, canonical, generation-bound, and transactionally
linked to the metadata publication it describes. Event order is not inferred
from wall-clock timestamps.

## Version

No M2 version entity exists. `branch_ref` is already a reserved workspace field
but is not a branch implementation. Commits, parent relations, refs with
version semantics, immutable history projections, and replay belong to later
phases. A snapshot must not be called a commit, checkpoint, or version.

## Restore Relationship

Restore consumes one verified Snapshot and creates a new local materialization.
The destination is intentionally new-only in the bounded slice. The source
workspace and snapshot remain immutable; restore does not move a branch, rewrite
events, or imply reversal of external effects. A restore operation/event records
verification and publication outcome. Existing destinations are conflicts;
post-rename sync uncertainty is retained for reconciliation.

The restore intent is an `Operation` with action `snapshot.restore` and a typed
snapshot input reference. Its terminal outcome and `snapshot.restore.completed`,
`.failed`, or `.unknown` domain event commit together. A completed result binds
the snapshot and canonical destination; retry may accept an existing directory
only for the same started/terminal operation and only after full manifest/blob
verification. Restore does not mutate workspace head or snapshot metadata.

## Lifecycle Operation Identity

M2-SLICE-009 reuses the existing operation ledger for lifecycle mutations. The
durable operation identity is `operation_id`; request retries use the existing
`(project_id, agent_id, request_id)` uniqueness constraint and canonical
envelope digest. A lifecycle operation envelope is bound to one workspace,
expected lifecycle status/revision, lease owner/epoch, and action. The existing
operation journal and operation event stream remain the audit trail.

For a mutating lifecycle action, operation intent, workspace status/revision
CAS update, terminal completed outcome, journal phase, and operation lifecycle
events commit atomically in the existing SQLite transaction. Exact retries
return the durable completed result; changed action, request, or expected
revision is rejected deterministically. Pre-commit interruption exposes the
old state with no operation row, while post-commit interruption exposes the
fully committed operation and workspace state for safe retry after reopen.
`Open`, status, and diff remain read-only and do not allocate operation or
lifecycle event records. No new event taxonomy, operation table, or provider
metadata is introduced.

## Snapshot Diff

M2-SLICE-004 adds a read-only, in-memory comparison of two verified snapshot
manifests. `SnapshotDiff` carries the old and new typed snapshot IDs plus
ordered `SnapshotDiffEntry` values. Entries use the existing canonical
relative paths and classify only `ADDED`, `REMOVED`, `MODIFIED`, and
`TYPE_CHANGED`; unchanged paths are omitted. File entries retain old/new
digest and size metadata, while directory entries carry no synthetic metadata.

The comparison validates both manifests through the existing workspace/project
and redaction identity boundary, then performs an O(N + M) sorted merge. It
does not read all file blobs, write CAS or SQLite, add events, or infer
permissions, timestamps, ACLs, inode numbers, symlink targets, or semantic
conflicts. A path-level diff from the current physical workspace to a snapshot
is added by M2-SLICE-005 as an ephemeral, read-only observation. It is not
persisted or used as a new head.

## Workspace Current-Tree Diff

`WorkspaceManager::diff_workspace` uses only the durable workspace head as its
reference. It verifies the head's snapshot metadata, operation, event,
generation, migration, counters, and canonical locator before scanning the
local tree. The current manifest is built in memory with the existing scanner
and receives an ephemeral `workspace-current-<digest>` identity. The output
reuses `SnapshotDiff` and deterministic canonical path ordering.

The scan is a `POINT_IN_TIME_OBSERVATION`: concurrent mutation can make
separate calls observe different trees, while a file changing during its own
read fails closed. `WorkspaceDiffResult` records the durable observation
revision, immutable environment binding, selected head, and
`observation_stability = "stable"` for successful results. Revision remains
the optimistic-concurrency/lifecycle revision, not a filesystem counter. The
workspace row is re-read after scanning; a detectable identity, locator, head,
environment, or revision change returns `CONFLICT/UNSTABLE_OBSERVATION`. No
watcher, lock, persistent diff table, CAS publication, SQLite mutation, event,
or revision advance is implied. A scan timestamp is intentionally omitted so
repeated results remain deterministic.

## Cardinality and Integrity

- Project 1--N Workspace.
- Workspace 0..1 current Environment binding.
- Workspace 0..1 current Snapshot head.
- Snapshot 1--1 canonical manifest; manifest 1--N immutable file blobs.
- Workspace 1--N Operations over time.
- Operation 1--N lifecycle Events.
- Event and metadata records share project/generation/redaction identity.

The following must fail closed: a manifest for another workspace/project, a
missing or mismatched blob, a snapshot metadata row with the wrong generation,
a stale lease/revision, a cross-project environment, non-canonical paths, or a
secret-containing persisted value.

## Not Modeled Yet

Workspace diff projections, checkpoints,
commits, branches, conflict records, task/agent registries, provider attach or
reconciliation, and public API serialization are intentionally absent from this
M2 data model.
