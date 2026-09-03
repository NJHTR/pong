# M2 Workspace and Snapshot Gate

**Status: bounded internal, test-gated M2 slice; not a public-release gate.**

This document is the acceptance boundary for the first M2 implementation slice.
It records executable behavior that exists in the Rust library while keeping
the larger workspace model (providers, lifecycle, reconciliation, commits, and
restore) explicitly out of scope. Passing this slice does not pass M1 and does
not authorize a public CLI, runtime, SDK, server, or UI.

## Scope of the current slice

The current Rust implementation provides:

- durable `workspaces`, `workspace_leases`, and `environments` metadata rows;
- logical workspace IDs with a local driver and a checked, redacted locator;
- one active write lease per workspace, with an owner, expiry, and increasing
  epoch; lease-guarded updates also require an expected workspace revision;
- deterministic, allowlisted environment facts (OS, architecture, family, and
  selected variables), with sensitive/host-identity keys and obvious
  secret-shaped values excluded before persistence;
- immutable environment identity: reusing an ID with different facts or a
  different project is an integrity failure;
- a local filesystem snapshot that rejects symlinks/reparse points, unsafe or
  non-portable relative paths, and non-regular entries;
- canonical, sorted tree manifests under `workspace/tree/v1` and immutable file
  blobs under `workspace/blob/v1`, with per-file-size and file-count limits;
- snapshot reads that check file size/mtime before and after reading; and
- materialization into a new temporary directory, followed by file and
  directory synchronization and atomic rename. An existing destination is a
  conflict and is never overwritten.

`WorkspaceManager` advances a workspace head only after the snapshot manifest
has been published to CAS and the metadata update succeeds under a valid lease.
An error may leave a verified but unreachable CAS object; it must not advance
the workspace head.

M2-SLICE-001 also persists a typed `snapshot_id`, the CAS root digest,
workspace/project/environment, manifest/redaction identities, counts,
operation/event IDs, and generation/migration identity in an additive
`snapshots` table. The metadata row, `snapshot.created` Event Envelope, and
workspace head/revision commit atomically. A synthetic post-commit error is
reconciled by an exact retry only after it verifies the complete existing
metadata/event/head/operation relation.

M2-SLICE-002 adds a durable new-destination restore operation. It verifies the
snapshot record, canonical manifest, blobs, generation identity, and canonical
destination before/while materializing. The terminal operation outcome and
`snapshot.restore.*` event share one metadata transaction. Failed publication,
post-rename sync uncertainty, cold reopen, and retry reconciliation are
explicit; existing user destinations are never overwritten.

M2-SLICE-003 adds a read-only local workspace status view. It validates the
 durable workspace/head/snapshot/environment/operation relations, reports lease
 activity at a caller-supplied time, and derives filesystem change state by
 comparing the current tree to the published head without writing CAS or
 changing metadata.

M2-SLICE-005 adds a read-only path-level current-tree diff against that same
durable head. It validates head metadata/event/generation identity, scans the
current local tree with the existing canonical safety checks, and reuses the
deterministic `SnapshotDiff` result. The current-tree identity is ephemeral;
the operation does not publish CAS, metadata, leases, revisions, operations,
or events. Scans are point-in-time observations without watcher or lock
semantics. M2-SLICE-006 binds the result to the observed durable revision,
head, and environment. The workspace row is re-read after scanning; a
detectable lifecycle/head change returns `CONFLICT/UNSTABLE_OBSERVATION`.

Workspace creation validates an optional environment ID against the same
project before the row is committed. The guarded transition to
`ready`/`active`/`paused` repeats the cross-row check, so a binding cannot become
valid merely because an environment row was later replaced or moved.

## Explicit non-goals

The slice does **not** yet provide:

- commit objects, branch movement, persistent diff/log projections, or restore into an
  existing workspace;
- attach/detach/migrate/freeze/resume/archive lifecycle transitions;
- crash-time provider reconciliation or a durable snapshot-intent journal;
- incremental snapshots, hard-link/reflink optimization, or a total-byte
  snapshot budget;
- preservation of symlinks, device files, sockets, ACLs, ownership, or file
  modes;
- process, network, database, container, remote, or sandbox capture/isolation;
- a provider-neutral public API or compatibility-stable SDK/CLI contract.

The local locator is currently stored as a canonical absolute path for the
local driver. It is metadata, not workspace identity, and paths inside the
repository control directory are rejected. Portability across machines will
require a later driver/materialization contract.

## Invariants for this slice

1. A workspace ID is logical and immutable; changing or reusing the physical
   path cannot create a second identity implicitly.
2. At most one unexpired lease is active. A stale owner, stale epoch, expired
   token, or wrong workspace receives a conflict and cannot mutate metadata.
3. Workspace revision updates are compare-and-swap operations. A stale
   expected revision cannot replace a newer head or status.
4. `ready`, `active`, and `paused` require both a head and an environment ID;
   an environment must exist and belong to the workspace project.
5. Environment IDs are immutable record identities with canonical content
   fingerprints. Equivalent facts are idempotent; changed facts fail closed.
6. Every manifest path is UTF-8, relative, portable, traversal-free, and
   canonically sorted. Manifest identity includes workspace, project, and
   redaction-profile versions.
7. A snapshot publishes file blobs before its manifest. Publication failure
   leaves no readable partial object and does not move the workspace head.
8. Materialization never writes into an existing destination. A failed or
   interrupted build can remove only its temporary directory; the destination
   remains absent or complete.
9. Configured secrets are rejected or redacted before locator, manifest, and
   metadata persistence. No test may assert safety from a post-persistence
   cleanup scan alone.
10. A durable snapshot publication is all-or-nothing: before commit it exposes
    the old head with no metadata/event; after commit, cold reopen exposes the
    matching metadata, event, head, and revision.
11. A restore publishes only to a new destination. Its durable terminal
    operation and domain event agree on completed/failed/unknown; an exact
    retry can reconcile an already-published destination only after full CAS
    and manifest verification.
12. A status query is read-only. It reports `changed` only as the current
    filesystem tree versus the published head; without a head it reports
    `change_state = "no_snapshot"` and does not infer a clean baseline.
13. A workspace diff uses only the durable head, fails closed for a missing or
    inconsistent head, reports deterministic path-level changes without
    advancing workspace state, and rejects a detectable head/revision/
    environment change around the scan as an unstable observation.

## Current evidence

`tests/workspace_snapshot.rs` currently covers:

- lease exclusivity, epoch takeover, stale lease rejection, and stale revision
  rejection;
- repeated snapshots producing one canonical digest and deterministic path
  ordering;
- local materialization and existing-destination conflict;
- manager head advancement under a lease;
- file-count and per-file-size limits;
- registered-secret rejection without head/revision advancement;
- missing and wrong-project environment binding rejection; and
- cold-reopen persistence with expired-lease takeover;
- Unix symlink rejection; and
- sensitive environment allowlist filtering.

The tests are local executable evidence only. They do not establish crash
durability, broad platform support, or a release API.

Additional bounded-slice evidence is in `tests/workspace_faults.rs` and
`tests/compatibility_boundaries.rs`:

- CAS blob/manifest publication faults leave no partial staging object and do
  not advance workspace head or revision, including a metadata commit fault;
- materialization write, file-sync, tree-sync, rename, parent-sync, and cleanup
  faults distinguish cleanup-safe failure from outcome-unknown publication;
- a cold-reopened workspace can explicitly remove only matching orphan
  temporary directories, while cleanup failure retains the orphan for retry;
- permission, resource-exhausted, and short-write actions preserve stable domain
  error codes; and
- a raw v0.1 SQLite fixture remains limited to legacy tables during source
  migration while the target generation receives additive M2 tables and passes
  a cold reopen.

`tests/snapshot_publication.rs` provides M2-SLICE-001 synthetic
metadata/head-atomicity and post-commit retry coverage. This is Windows local
development evidence, not a cross-platform M2 release qualification.

`tests/durable_restore.rs` provides M2-SLICE-002 success, cold-reopen,
idempotent retry, existing-destination, missing-blob, materialization-fault,
parent-sync unknown, and post-materialization metadata-retry coverage. The
bounded slice is `PASS / INTERNAL / TEST-GATED` on retained Windows local NTFS
development evidence; this is not the M2 exit gate or cross-platform evidence.

`tests/workspace_status.rs` provides M2-SLICE-003 status coverage for no-head
and published/changed trees, restore and unresolved operations, lease expiry and
takeover, corruption fail-closed behavior, cold reopen, serialization, and a
raw v0.1-style no-snapshot workspace. The retained status record is
[`m2-slice-003-workspace-status-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-003-workspace-status-windows-native-2026-09-01.json).
The Windows local NTFS development run recorded `cargo fmt --all -- --check`,
`cargo check --locked`, the focused M2 suites, an independent-target
`cargo test --all --locked` result of 174 passed / 2 ignored / 0 failed, and
`cargo clippy --all-targets --all-features --locked -- -D warnings` with exit
code 0. The ignored FI-13 host-resource and measurement-only tests remain
ignored and are not counted as PASS evidence.

`tests/workspace_diff.rs` provides M2-SLICE-005 coverage for clean, added,
removed, modified, file/directory type changes, deterministic repeated scans,
10,000 files, corrupt references, missing heads, cold reopen, head identity
corruption, and point-in-time mutation. The retained record is
[`m2-slice-005-workspace-diff-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-005-workspace-diff-windows-native-2026-09-01.json).
This is Windows local native / NTFS development evidence only and is not
cross-platform or release qualification.

The focused evidence is still local and synthetic where failpoints are used;
it does not close the declared-platform, host-resource, performance, or old
binary matrices required by the exit gate.

## Exit requirements

M2 cannot pass until all of the following are recorded:

- `cargo fmt`, `cargo check`, unit/integration tests, and clippy on the pinned
  Rust 1.78 toolchain and each declared release platform;
- cold-reopen tests proving workspace, environment, lease epoch, expiry, and
  revision behavior after process termination;
- focused path/limit tests for traversal, reserved names, invalid UTF-8,
  symlink/reparse entries, file-count and per-file-size limits, and a registered
  secret in a workspace file; each failure must leave head/revision unchanged;
- wrong-project and missing-environment binding tests, including the create and
  update paths;
- fault schedules for CAS ENOSPC/permission, a file changing during read,
  materialization write/sync/rename, and a crash before workspace-head update,
  with cold-restart and temporary-directory inspection;
- a reconciliation decision for unreachable CAS objects and abandoned
  materialization directories;
- measured snapshot/materialization cost and bounded-resource behavior on the
  supported filesystem matrix; and
- updated architecture, security, reliability, roadmap, and changelog docs,
  plus an ADR for any change to the logical workspace or lease contract.

M1's durable-primitives gate remains independently open. Evidence in this
document may be used to develop M2, but it cannot be used to declare the M1
release gate passed.

## References

- [`WORKSPACE_MODEL.md`](../architecture/WORKSPACE_MODEL.md)
- [`ENVIRONMENT_MODEL.md`](../architecture/ENVIRONMENT_MODEL.md)
- [`STORAGE_ARCHITECTURE.md`](../architecture/STORAGE_ARCHITECTURE.md)
- [`M1_EVIDENCE.md`](M1_EVIDENCE.md)
- [`NEXT_TASK.md`](../roadmap/NEXT_TASK.md)
