# M2 Workspace and Snapshot Gate

**Status: bounded internal slice frozen until M1 passes; gate not passed.**

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

Workspace creation validates an optional environment ID against the same
project before the row is committed. The guarded transition to
`ready`/`active`/`paused` repeats the cross-row check, so a binding cannot become
valid merely because an environment row was later replaced or moved.

## Explicit non-goals

The slice does **not** yet provide:

- commit objects, branch movement, diff/log projections, or operation-ledger
  links;
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
