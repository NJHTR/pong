# M2 Entry Audit

**Audit date:** 2026-08-30
**M1 baseline:** `v0.1.0` at `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`
**Branch/working tree:** `dev`, clean at audit start
**M1 status:** released baseline; M2 remains an internal, test-gated phase

This document is an entry audit, not an implementation approval and not a
public API promise. It records what is present in the repository and what M2
still has to define or prove.

## M2 Goal

Phase 2 of the roadmap is the local workspace and snapshot driver. The goal is
to make a logical workspace safely observable and materializable as an
immutable, content-addressed tree while preserving the M1 durability,
generation, event, projection, and recovery contracts.

## M2 Scope

The roadmap and `M2_WORKSPACE_SNAPSHOT_GATE.md` define this bounded scope:

- local filesystem workspace driver;
- logical workspace identity and metadata;
- leases and revision-guarded workspace mutation;
- safe, redacted environment facts;
- portable tree snapshot manifests and CAS file blobs;
- snapshot materialization into a new directory;
- validation of paths, object kinds, limits, secrets, and project bindings;
- crash/fault/reconciliation evidence for the above.

## M2 Non-goals

The following are not part of this entry or the first slice: commits,
branches, version DAGs, checkpoints, semantic diff/log projections, provider
reconciliation, process/network/database capture, non-local drivers,
incremental snapshots, hard-link/reflink optimization, public CLI/SDK, server,
UI, or framework adapters. Those capabilities remain governed by later
roadmap phases or explicit future ADRs.

## Baseline and Compatibility Constraints

- M1 `v0.1.0` is immutable baseline; its tag and release artifacts are not
  rewritten.
- Existing Event Envelope, Projection Contract, project sequence/cursor,
  operation ledger, generation identity, migration selector, and recovery
  semantics are inputs to M2, not redesign targets.
- Additive M2 metadata/object domains must remain invisible to a legacy reader
  unless a future migration/compatibility decision explicitly says otherwise.
- A contract change requires a new `ADR-M2-*`; this audit does not accept or
  modify ADR-0015 or ADR-0016.

## Implementation Audit

| Area | Status | Existing repository fact | Missing or incomplete |
| --- | --- | --- | --- |
| Workspace identity | `EXISTS` | `WorkspaceRecord.workspace_id` is logical and project-scoped; locator is metadata. | Public identity contract and stable ID format are not frozen. |
| Workspace create/open/close | `PASS (M2-SLICE-007 contract)` | `WorkspaceManager::create_local`, `LocalWorkspace::create/open`, and the provider-neutral lifecycle validator exist. `Open` is read-like/idempotent; `Close` is logical archive only. | Durable lifecycle orchestration and physical cleanup remain provider responsibilities; no public API. |
| Workspace metadata | `EXISTS` | SQLite stores driver, locator, branch ref, head, environment, status, revision and timestamps. | Schema evolution and query/list API are not an M2 release contract. |
| Workspace leases | `EXISTS` | Epoch lease acquire/renew/release and stale-token checks exist. | Public clock, multi-process protocol, skew policy and lifecycle integration remain open. |
| Workspace status | `IMPLEMENTED (M2-SLICE-003)` | `WorkspaceManager::status` returns a read-only serializable view of identity, revision, head, lease, environment, filesystem change state, execution readiness, and recent/unresolved operations. | No user-facing CLI projection, full lifecycle transition graph, or background status index. |
| Environment binding | `EXISTS` | Allowlisted/redacted facts and same-project immutable environment identity exist. | Reproducibility guarantee and complete provider fingerprint are explicitly out of scope. |
| File change detection | `IMPLEMENTED (M2-SLICE-005)` | Current-tree scan reuses the status/snapshot walker, validates size/mtime around reads, and returns an in-memory canonical manifest. | No watcher, lock, or globally consistent view during concurrent mutation. |
| Snapshot creation | `IMPLEMENTED (M2-SLICE-001)` | Canonical sorted tree manifest plus immutable file blobs are written to typed CAS domains; publication persists metadata/head/event atomically. | Public creation contract remains intentionally out of scope. |
| Snapshot identity | `IMPLEMENTED (M2-SLICE-001)` | `snapshot_id` is `snp-<tree-digest>` and is distinct from the `sha256:<tree-digest>` CAS root, workspace revision, operation ID, and event ID. | Public identity contract and future namespace evolution remain unfrozen. |
| Snapshot metadata | `IMPLEMENTED (M2-SLICE-001)` | Additive `snapshots` rows retain workspace/project/environment, manifest/redaction, counts, operation/event, generation, and migration identity. | Parent/base, retention, and public query contracts remain out of scope. |
| Snapshot integrity | `EXISTS` | Canonical bytes, manifest validation, blob digest/size checks and CAS verification exist. | Cross-restart evidence for all declared platforms and reconciliation policy still need M2 execution. |
| Snapshot-CAS relation | `EXISTS` | Manifest references `workspace/blob/v1` objects; manifest itself is `workspace/tree/v1`. | Reachability/retention and orphan policy are not complete. |
| Snapshot restore | `IMPLEMENTED (M2-SLICE-002)` | A verified snapshot materializes to a new destination with a durable operation, linked completion/failure/unknown event, cold-reopen verification, and idempotent reconciliation. | Restore into an existing workspace, safety snapshots, and public policy/API remain out of scope. |
| Workspace diff | `IMPLEMENTED (M2-SLICE-005)` | `WorkspaceManager::diff_workspace` validates only the durable head and compares the current local tree with deterministic `SnapshotDiff` entries. | No persistent diff projection, watcher, public API, or non-local driver. |
| Snapshot diff | `IMPLEMENTED (M2-SLICE-004)` | `LocalWorkspace::diff_snapshots` validates both manifests and performs a deterministic O(N + M) path merge with added/removed/modified/type-changed results. | Public contract and large-scale release qualification remain out of scope. |
| Operation identity/lifecycle | `EXISTS` (internal) | M3 bounded ledger has immutable envelope, idempotent start/finish and terminal states. | It is not yet the M2 snapshot/restore public contract. |
| Operation-event relation | `IMPLEMENTED (M2-SLICE-001/002, internal)` | Snapshot publication and restore outcomes are linked to durable operations with workspace, causation, correlation, generation, and snapshot identity. | Public operation/event contracts remain unfrozen. |
| Version identity/parents | `MISSING` | No commit/version/branch object or parent graph exists in `src`. | Planned for Phase 4; do not invent it in M2. |
| Version replay/immutable history | `MISSING` | Event history is immutable, but version replay is not implemented. | Planned for later recovery/version phases. |
| Recovery of workspace/snapshot | `PARTIAL` | M1 recovery, materialization fault tests, and restore cold-reopen/retry reconciliation exist. | General provider reconciliation and restore-into-existing-workspace recovery remain out of scope. |

## Existing Tests

The repository already contains local executable coverage in
`tests/workspace_snapshot.rs` and `tests/workspace_faults.rs` for lease
exclusivity/epoch takeover, stale revisions, deterministic manifests,
materialization conflicts, limits, redaction, project bindings, symlink or
reparse rejection, CAS publication faults, materialization write/sync/rename
faults, orphan cleanup, and cold reopen. `tests/compatibility_boundaries.rs`
also covers additive M2 tables during migration. The M1 suite remains the
regression suite.

`tests/workspace_status.rs` covers no-head status, published/changed trees,
lease expiry, restore/operation summaries, unresolved operations, head and
environment corruption, cold reopen, stable JSON serialization, and a
legacy-style workspace with no snapshot. The status query is read-only and
does not advance revision or mutate leases.

`tests/workspace_diff.rs` covers the internal current-tree-to-head diff:
clean, added, removed, modified, file/directory type changes, deterministic
multiple changes, repeated scans, 10,000 files, corrupt reference manifests,
missing heads, cold reopen, head identity corruption, and the explicit
point-in-time mutation limitation.

## M2-SLICE-001: Atomic Snapshot Publication

**Status:** `PASS / INTERNAL / TEST-GATED`.

The bounded slice adds only the durable publication boundary:

```text
verified CAS tree
  -> snapshots metadata row
  -> snapshot.created envelope
  -> workspace head + revision
  -> SQLite commit
```

The metadata row, event, head, and revision share one SQLite transaction.
Pre-commit synthetic faults retain the complete pre-state. A post-commit
synthetic fault is outcome-unknown to the initial caller; an exact retry
verifies the existing metadata/event/head/operation relation and converges
without a second snapshot publication. Cold reopen verifies the durable
post-state. CAS objects are written first, so a failed metadata transaction
may leave an unreachable verified object but cannot publish a dangling head.

`tests/snapshot_publication.rs` covers successful publication, pre-commit
metadata/head failures, post-commit cold reopen, and idempotent retry. The
raw v0.1 compatibility fixture proves `snapshots` is additive in a migration
target while the source remains unchanged. These are local development facts,
not public API, M2 release, or cross-platform qualification evidence.

## M2-SLICE-005: Workspace Current-Tree Diff

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::diff_workspace` resolves the workspace's durable head,
verifies its snapshot metadata and event relations, opens the canonical local
locator, scans the current tree in memory, and delegates comparison to the
existing deterministic manifest merge. It publishes no CAS object and mutates
no metadata. Missing heads and inconsistent identities fail closed.

The retained run is
[`m2-slice-005-workspace-diff-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-005-workspace-diff-windows-native-2026-09-01.json).
It is Windows local native / NTFS development evidence only, is bound to an
uncommitted development tree, and is not M2 release or cross-platform
qualification evidence.

## M2-SLICE-006: Workspace Diff Reconciliation / Lifecycle

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::diff_workspace` now returns a point-in-time observation
bound to the durable head, workspace lifecycle revision, and environment ID.
It validates the same-project environment binding before the scan and re-reads
the workspace row after scanning. A detectable head, revision, environment,
locator, or identity change returns the existing `CONFLICT` domain error with
`UNSTABLE_OBSERVATION`; no watcher, lock, durable diff state, operation, event,
CAS write, or revision mutation was added. A scan timestamp remains outside
the current deterministic result model.

`tests/workspace_diff_reconciliation.rs` covers snapshot publication followed
by an empty diff, restored materialization comparison and post-restore change,
repeatability, read-only metadata/event preservation, environment mismatch,
cold reopen, durable head changes, legacy readability, and a concurrent test
harness revision change detected after a 10,000-entry scan. The retained
Windows local NTFS development record is
[`m2-slice-006-workspace-diff-reconciliation-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-006-workspace-diff-reconciliation-windows-native-2026-09-01.json).
This is internal development evidence, not M2 release qualification.

## M2-SLICE-007: Provider-Neutral Workspace Lifecycle Contract

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceLifecycleState`, `WorkspaceLifecycleAction`,
`WorkspaceCapabilities`, `WorkspaceIdentity`, and
`WorkspaceProviderContext` define the smallest provider-neutral boundary over
the existing seven durable lifecycle states. `Open` is handle acquisition and
does not advance durable status; `Close` is logical archive and does not imply
physical directory deletion. Capabilities are limited to `snapshot`,
`restore`, `diff`, and `status`.

`transition_workspace_lifecycle` and `require_workspace_capability` are pure
internal validators. Provider-specific paths, filesystem facts, handles, and
metadata are excluded from the context. Provider success followed by core
persistence failure, provider failure, interruption/recovery, deterministic
retry, and unsupported capability behavior are covered by the explicit
`TEST_DOUBLE` in `tests/workspace_lifecycle_contract.rs`.

The L1-L15 suite passed on Windows 11 / NTFS local development. The retained
record is
[`m2-slice-007-workspace-lifecycle-contract-windows-native-2026-09-02.json`](../../artifacts/m2-development/m2-slice-007-workspace-lifecycle-contract-windows-native-2026-09-02.json).
This is not a non-local provider qualification, public Rust/SDK contract, or
M2 release gate. No SQLite schema, M1 contract, ADR-0016, provider registry,
Git/remote provider, or physical close guarantee was added.

## M2-SLICE-008: Durable Workspace Lifecycle Transition Integration

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::transition_lifecycle` integrates the S007 pure transition
validator with the existing `MetadataStore::update_workspace` path. Mutation
actions validate workspace identity, transition legality, lease owner/epoch/
expiry, and expected revision before the existing immediate SQLite transaction
increments revision and persists the durable status. `Open` remains read-like
and does not mutate revision. Exact retries of a completed state-changing
transition are recognized deterministically without a second increment.

No lifecycle operation or event is synthesized because the existing workspace
update contract has no accepted lifecycle event type; snapshot/restore retain
their existing operation/event linkage. Pre-commit and post-commit metadata
failpoints retain the existing pre-state or durable post-state semantics.
Cold reopen reads the durable state and revision without inferring provider
effects. The integration suite also verifies stale revision/lease rejection,
provider failure without core mutation, read-only status/diff, no phantom
success, and legacy v0.1 readability.

The retained record is
[`m2-slice-008-workspace-lifecycle-integration-windows-native-2026-09-02.json`](../../artifacts/m2-development/m2-slice-008-workspace-lifecycle-integration-windows-native-2026-09-02.json).
This is Windows local native / NTFS development evidence only. No SQLite
schema, M1 contract, ADR-0016, provider implementation, or public API was
added.

## M2-SLICE-009: Lifecycle Operation Identity Integration

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::transition_lifecycle_operation` reuses the existing
operation ledger: `operation_id` is the durable operation identity and
`(project_id, agent_id, request_id)` is the request idempotency key guarded by
the canonical envelope digest. The operation is bound to one workspace,
expected lifecycle status/revision, lease owner/epoch, and action.

Operation intent, workspace status/revision CAS, terminal completed result,
journal phase, and operation lifecycle events commit in one existing SQLite
transaction. Exact retries return the same completed operation; changed action,
request, or expected revision fails closed. Pre-commit faults roll back both
operation and workspace, while post-commit faults leave a fully committed
state safe to retry after reopen. `Open`, status, and diff remain read-only and
do not allocate operation records or lifecycle events. No schema or M1 event
taxonomy was added.

The focused durable suite is
[`workspace_lifecycle_operation.rs`](../../tests/workspace_lifecycle_operation.rs)
(8 passed). Evidence is retained in
[`m2-slice-009-workspace-lifecycle-operation-windows-native-2026-09-02.json`](../../artifacts/m2-development/m2-slice-009-workspace-lifecycle-operation-windows-native-2026-09-02.json).
This remains Windows local native / NTFS development evidence only.

## Remaining Tests and Evidence

- non-local provider qualification and broader lifecycle orchestration;
- non-local provider qualification and durable lifecycle orchestration;
- restore-into-existing-workspace and safety-snapshot policy, outside the
  bounded new-destination slice;
- property corpus for manifest identity, path invariants, restore idempotency,
  and no mixed generation state;
- native supported-platform and filesystem evidence for the M2-specific rows;
- measured snapshot/materialization cost and bounded resource behavior;
- explicit orphan CAS retention/quarantine policy and evidence.

## Recommended Vertical Slice

The smallest useful M2 slice is:

```text
open Repository
  -> open/create logical Workspace under a lease
  -> capture immutable Snapshot (CAS + durable snapshot metadata)
  -> append snapshot event linked to the operation
  -> restore Snapshot to a new, verified materialization
  -> cold-reopen and verify identity, integrity, and workspace head
```

This slice should not introduce commits, branches, public SDKs, or provider
abstractions beyond the existing local driver. It must preserve the existing
workspace-head compare-and-swap rule and make any post-publication uncertainty
explicit.

## Entry Decision

`M2_ENTRY = READY FOR DESIGN; M2-SLICE-001/002/003/004/005/006/007 = PASS (INTERNAL); M2 = NOT READY FOR PUBLIC RELEASE`.

Snapshot publication and durable new-destination restore are implemented.
Workspace status is implemented as a read-only internal view. Diff,
restore-into-existing-workspace policy, M2 property/fault matrices,
supported-platform evidence, performance evidence, durable lifecycle
orchestration, and public API decisions remain separate work.

## M2-SLICE-002: Durable Restore

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::restore_local` verifies durable snapshot metadata, typed CAS
manifest/blob integrity, workspace/project/generation identity, and a canonical
destination outside `.pong`. It records the restore intent before filesystem
publication and commits the terminal outcome plus `snapshot.restore.*` event
atomically. The destination is new-only and is never overwritten.

Pre-publication materialization faults are durable failures. A parent-sync
fault after rename is durable `unknown`. If terminal SQLite persistence fails
after publication, an exact retry verifies the existing destination and
completes the original operation. `tests/durable_restore.rs` covers these
boundaries plus cold reopen, duplicate-event prevention, missing CAS blobs, and
existing-destination safety. These are local development facts, not M2 release
or public API qualification. The retained Windows local NTFS development record
is
[`m2-slice-002-durable-restore-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-002-durable-restore-windows-native-2026-09-01.json).

## M2-SLICE-003: Workspace Status

**Status:** `PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::status(workspace_id, now_ms)` is a point-in-time,
read-only query. It validates the local locator, workspace lifecycle and
revision, environment/project binding, head-to-snapshot metadata and event
relation, generation identity, canonical manifest, and CAS blobs before
returning a `WorkspaceStatus` value. Filesystem change state is derived with
semantic B: the current regular-file tree is compared to the current published
head snapshot in memory. No head means `changed = None` and
`change_state = "no_snapshot"`.

Lease activity is derived from the durable owner/epoch/expiry row and the
caller-supplied time; status never acquires, renews, releases, or mutates a
lease. A started or unknown workspace operation sets `recovery_required` and
prevents `healthy`/`execution_ready` from being reported. Corrupt head,
snapshot, environment, manifest, or CAS relations fail closed with existing
domain errors. The retained evidence is
[`m2-slice-003-workspace-status-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-003-workspace-status-windows-native-2026-09-01.json).

## M2-SLICE-004: Deterministic Snapshot Diff

**Status:** `PASS / INTERNAL / TEST-GATED`.

`LocalWorkspace::diff_snapshots` compares two verified `workspace/tree/v1`
manifests belonging to the same workspace, project, and redaction profile.
It reuses `read_manifest`, emits only `ADDED`, `REMOVED`, `MODIFIED`, and
`TYPE_CHANGED` entries, and sorts paths with the existing canonical byte
ordering. Directory entries follow the current manifest semantics; unchanged
parent directories are omitted, while added/removed directories and
file/directory replacements remain visible.

`tests/deterministic_diff.rs` covers empty, add, remove, modify, type-change,
multiple-change, repeatability/serialization, 10,000-entry, corrupt-manifest,
cold-reopen, and raw v0.1 repository compatibility cases. The operation reads
manifest metadata only and does not load every referenced file blob. This is a
Windows local NTFS development slice; its synthetic-manifest complexity sanity
check covers 100, 1,000, 10,000, and 100,000 entries. It is not a public API or
M2 release gate.
