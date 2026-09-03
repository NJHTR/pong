# ADR-M2-005: Workspace Current-Tree Diff

- **Status:** Proposed / internal M2 slice
- **Date:** 2026-09-01
- **Baseline:** M1 `v0.1.0`, commit `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

## Context

`M2-SLICE-004` compares two immutable snapshot manifests. A local workspace
also needs a read-only view of changes made since its durable snapshot head.
This slice supplies that point-in-time observation without publishing a new
snapshot or introducing a persistent diff projection.

## Contract

`WorkspaceManager::diff_workspace(workspace_id)` reads only the workspace's
durable `head`. The head must resolve to a snapshot metadata row whose
workspace, project, environment, generation, migration, manifest counters,
operation, and `snapshot.created` event all agree. A missing head is an
integrity error; another snapshot is never selected implicitly.

For a local workspace, the existing scanner builds an in-memory canonical
`TreeManifest` using the same path, reparse-point, redaction, file-count, and
file-size checks used by status/snapshot capture. The reference manifest is
read and validated from `workspace/tree/v1` CAS. The existing deterministic
`SnapshotDiff` schema then compares reference entries with current entries.

## Observation Semantics

The scan is point-in-time only. File size and modification time are checked
around each file read, and a file that changes during that read fails closed.
There is no watcher, filesystem transaction, lock, or claim of a globally
consistent view while another process mutates the tree. Repeated calls are
independent observations and do not mutate CAS, SQLite, workspace revision,
leases, operations, or events.

The current tree receives an ephemeral identity of the form
`workspace-current-<manifest-digest>`. It is not a snapshot, does not enter
metadata, and cannot become a workspace head.

## Change and Ordering Rules

The output reports only `ADDED`, `REMOVED`, `MODIFIED`, and `TYPE_CHANGED`
entries, using the existing canonical relative paths and byte-lexicographic
ordering. Directory descendants are compared as ordinary entries. No
timestamps, permissions, ACLs, inode data, symlink targets, or semantic
conflicts are inferred.

## Errors and Safety

Missing/corrupt/non-canonical reference manifests, invalid paths, unsafe
entries, symlinks/reparse points, unsupported filesystem entries, redaction
violations, inconsistent head metadata, and incompatible generation identity
return the existing domain errors. A failed scan never advances workspace
state. Non-local workspace drivers return `UNSUPPORTED` for this local slice.

## Compatibility and Scope

This is additive and internal/test-gated. It changes no SQLite schema, event,
projection, migration, selector, or M1 compatibility contract. It does not
define restore-into-existing-workspace, diff persistence, watchers, locks,
versions, branches, commits, merges, CLI/SDK output, or non-local drivers.

## Performance

The manifest merge is O(N + M) after the current tree walk. The walk is
bounded by existing snapshot options and reads file bytes to compute current
content digests. The retained Windows NTFS development run records 100,
1,000, and 10,000-entry scan samples as measurement only; it is not an M2
release budget or cross-platform qualification.

## Acceptance

Acceptance for this bounded slice requires focused tests for clean, added,
removed, modified, type-changed, multiple-change, repeatability, 10,000-file,
corrupt-reference, missing-head, cold-reopen, identity-corruption, and
point-in-time mutation behavior, plus a retained local development run.
This ADR remains Proposed until a later owner decision.
