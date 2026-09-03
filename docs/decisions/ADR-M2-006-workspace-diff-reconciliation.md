# ADR-M2-006: Workspace Diff Reconciliation

- **Status:** Proposed / internal M2 slice
- **Date:** 2026-09-01
- **Baseline:** M1 `v0.1.0`, commit `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

## Context

M2-SLICE-005 provides a read-only current-tree diff against the durable
workspace head. A filesystem scan is necessarily point-in-time, while the
workspace row can be published by another lifecycle operation during that
scan. The result must identify which durable state it observed and must not be
reported as stable when that state changes around the scan.

## Decision

`WorkspaceManager::diff_workspace` is a `POINT_IN_TIME_OBSERVATION`. A
successful `WorkspaceDiffResult` records the logical workspace and project,
the durable head snapshot, `observation_revision`, immutable `environment_id`,
`observation_stability = "stable"`, and the ephemeral current-tree identity
and deterministic `SnapshotDiff`.

The revision is the existing optimistic-concurrency/lifecycle revision. It is
not a filesystem observation counter and is not incremented by diff. The head
continues to be the durable snapshot publication reference. A scan timestamp
is intentionally not included: the current internal model has no caller clock
contract, and adding wall-clock data to the equality-tested result would make
repeated observations non-deterministic.

Before scanning, the implementation validates the workspace/head/snapshot,
operation, event, generation, manifest, CAS counters, canonical locator, and
same-project environment binding. After scanning, it re-reads the workspace
row. Any change to workspace identity, project, driver, locator, head,
environment, or revision returns the existing `PongError::Conflict` with an
`UNSTABLE_OBSERVATION` diagnostic. A workspace that changes and changes back
cannot be detected without a watcher or lock and remains within the declared
point-in-time limitation.

## Lifecycle and mutation boundary

Diff is an observation only. It does not acquire or renew a lease, advance a
revision or head, write CAS, create an operation, or append an event. Snapshot
publication remains the mutation that advances head and revision; after
publication a new diff reads the new head. Restore remains a new-destination
mutation and does not alter the source workspace head. A restored destination
can be opened as a local workspace and compared with the snapshot using the
existing `LocalWorkspace` diff boundary.

No filesystem watcher, background worker, persistent diff projection, or
global lock is introduced. File size/mtime checks during individual reads
continue to fail closed using the existing integrity error taxonomy.

## Compatibility and scope

The result fields are additive to the internal/test-gated Rust model. No
SQLite schema, event, projection, generation, migration, M1 contract, public
API, CLI, SDK, version, branch, commit, or merge contract changes. A legacy
v0.1 repository remains readable; it is not required to understand the
additive result metadata.

## Acceptance evidence

`tests/workspace_diff_reconciliation.rs` covers clean and changed trees,
snapshot publication/head advancement, restore materialization comparison,
repeatability, read-only state preservation, environment mismatch, scan-time
revision change, cold reopen, and reading the durable new head. The retained
run is local Windows development evidence only. The scan-time revision test
uses a direct SQLite mutation in a test harness to model a concurrent
lifecycle writer; it is synthetic coordination evidence, not a production
bypass or host-resource claim.
