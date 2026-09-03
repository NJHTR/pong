# ADR-M2-004: Deterministic Snapshot Diff

- **Status:** Proposed / internal M2 slice
- **Date:** 2026-09-01
- **Baseline:** M1 `v0.1.0`, commit `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

## Context

Pong snapshots are immutable, canonical tree manifests stored in the existing
CAS. A caller needs to inspect the factual difference between two verified
snapshot states without turning a snapshot into a commit, branch, or semantic
conflict record.

## Goals

- Compare two snapshots belonging to the same local workspace/project and
  redaction profile.
- Return a deterministic, structured list of path changes.
- Reuse the existing manifest parser, identity checks, and path contract.
- Keep the operation read-only and linear in the number of manifest entries.

## Non-goals

This ADR does not define workspace watchers, a persistent workspace diff
projection, commits, versions, branches, merges, semantic conflicts, provider
state, a public API, CLI/SDK output, or a new metadata table.

## Diff Inputs

The internal `LocalWorkspace::diff_snapshots` operation accepts two
`workspace/tree/v1` CAS digests. Each manifest is read and validated with the
workspace's existing `read_manifest` path, identity, canonical-byte, and
redaction checks. Both snapshots must belong to the same workspace and
project. The manifests are authoritative for the comparison; the operation
does not read every file blob merely to classify a change.

## Diff Output

`SnapshotDiff` contains the old and new typed snapshot IDs and an ordered list
of `SnapshotDiffEntry` values. Each entry contains the canonical relative
path, a `SnapshotChangeType`, and optional old/new digest, size, and kind
metadata. Unchanged paths are omitted.

## Change Classification

- `ADDED`: only the new manifest contains the path.
- `REMOVED`: only the old manifest contains the path.
- `TYPE_CHANGED`: both contain the path but their kinds differ.
- `MODIFIED`: both are files and digest or size differs.

Directory entries are part of the existing manifest semantics. A directory is
reported when it is itself added or removed; unchanged parent directories are
not reported. A directory entry is not `MODIFIED` because the current tree
contract carries no directory metadata. A file/directory replacement is one
`TYPE_CHANGED` entry at that path; independent descendants are classified by
their own paths.

## Ordering and Determinism

Entries are sorted by the existing canonical path byte ordering used when
publishing manifests (`path.as_bytes()` lexicographic order). No hash-map
iteration order, filesystem enumeration order, or scheduling decision affects
the result. The same verified input digests produce byte-for-byte equivalent
serialized output.

## Identity and Metadata

Snapshot IDs use the existing `snp-<tree-digest>` convention. Digest strings
retain the existing `sha256:` prefix. The diff does not infer timestamps,
permissions, Windows ACLs, inode numbers, symlink targets, or other
OS-specific metadata because those fields are not in the current tree
manifest contract. Size and digest are the complete file metadata comparison.

## Symlinks and Unsupported Entries

Manifests are accepted only through the existing validator. Symlinks/reparse
points and non-regular entries cannot enter a valid manifest and therefore do
not receive a special diff classification.

## Integrity and Errors

Missing or corrupt manifest CAS objects, non-canonical bytes, incompatible
workspace/project/redaction identity, unsafe paths, duplicate paths, invalid
digest syntax, and unsupported entry kinds fail closed with the existing
`PongError` domain errors. Referenced file blobs are not loaded solely for
diff classification; materialization and status retain their existing full
blob verification boundaries.

## Compatibility

The operation is additive and read-only. A raw `v0.1` repository remains
openable and unchanged; it may be used with manifests available in the active
CAS but no legacy reader is required to understand a diff result. No SQLite
schema, event envelope, projection, generation, or migration contract is
changed.

## Complexity

After manifest validation, comparison uses one sorted union traversal: O(N + M)
time and O(N + M) output/manifest memory for manifests with N and M entries.
It does not read file contents or introduce parallelism.

## Acceptance

Acceptance for this bounded slice requires focused tests for empty, add,
remove, modify, type-change, multiple changes, deterministic ordering,
10,000-entry input, corrupted manifest fail-closed behavior, cold reopen, and
legacy repository compatibility, plus a recorded local development run. This
ADR remains Proposed until a later owner decision; its implementation is
internal and test-gated.
