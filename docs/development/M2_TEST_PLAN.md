# M2 Test Plan

**Status:** active test plan for the bounded internal M2 slice.
**Baseline regression:** M1 `v0.1.0` at `2aab0aaf4c9ddb342939da17eddb11de4dfa66c1`

M2 work is acceptable only if the complete M1 suite remains passing. Tests are
written for each new invariant before the corresponding implementation is
expanded. Synthetic failpoints are labeled synthetic; native and hosted
platform evidence are retained separately.

## M2-SLICE-001 Executed Coverage

`tests/snapshot_publication.rs` is the executable coverage for atomic snapshot
publication. It proves:

- a verified CAS tree receives durable snapshot metadata, a `snapshot.created`
  Event Envelope, and a workspace-head/revision update;
- the metadata row, event, head, and revision remain consistent after cold
  reopen;
- `BeforeSnapshotMetadataInsert` and `BeforeSnapshotHeadUpdate` are
  **SYNTHETIC** pre-commit faults that retain the complete old state; and
- `AfterSnapshotPublicationCommit` is a **SYNTHETIC** post-commit fault whose
  exact retry converges to one metadata row/event/head and completes the
  original operation.

`tests/compatibility_boundaries.rs` additionally proves that `snapshots` is an
additive target-generation table for the raw v0.1 migration fixture. The
executed evidence is Windows local native / NTFS development evidence only; it
does not qualify Linux or macOS and does not define a public API.

## M2-SLICE-002 Executed Coverage

`tests/durable_restore.rs` proves the bounded new-destination restore contract:

- verified manifest/blob bytes are materialized and linked to one durable
  completed operation and `snapshot.restore.completed` event;
- cold reopen and exact retry return the same result without duplicating the
  operation or event;
- an existing destination is preserved and recorded as a failed restore;
- missing CAS blobs and synthetic materialization faults fail without a
  successful destination;
- a synthetic parent-directory sync fault after rename records `unknown` and
  preserves the verifiable destination; and
- a synthetic terminal-metadata pre-commit fault leaves `started`, after which
  retry verifies the destination and commits the original result.

This is Windows local development coverage. It is not cross-platform M2
qualification and does not freeze a public restore API. The retained record is
[`m2-slice-002-durable-restore-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-002-durable-restore-windows-native-2026-09-01.json);
it binds an uncommitted development tree, reports the two existing opt-in tests
as ignored rather than passed, and does not claim MSRV execution.

## M2-SLICE-003 Executed Coverage

`tests/workspace_status.rs` proves the internal status view:

- a new workspace reports authoritative identity/revision, an active lease,
  bound environment, and `no_snapshot` with `changed = null` without mutation;
- a published head reports its typed snapshot identity and unchanged content,
  while a point-in-time file edit reports `changed = true` without publishing
  a new CAS object or advancing revision;
- latest operation and started/unknown recovery state are visible, and an
  expired lease is inactive even though its epoch tombstone remains durable;
- missing head metadata and cross-project environment bindings fail closed;
- cold reopen returns the same serializable status; and
- no-head legacy-style workspaces remain queryable.

This is Windows local NTFS development evidence only. The status API remains
internal/test-gated and is not a public CLI/SDK contract.

## M2-SLICE-004 Executed Coverage

`tests/deterministic_diff.rs` proves the internal snapshot-to-snapshot diff
boundary:

- identical snapshots produce an empty result;
- added, removed, modified, and file/directory type changes are classified
  with old/new digest, size, and type metadata;
- multiple changes omit unchanged entries and use canonical path-byte order;
- repeated runs serialize identically;
- a 10,000-entry manifest diff completes with all entries retained;
- a corrupted manifest fails closed before comparison;
- cold reopen preserves the same result; and
- a raw v0.1-style repository remains openable while the diff capability stays
  additive.

The same focused test records synthetic-manifest diff samples at 100, 1,000,
10,000, and 100,000 entries. These measurements are a local complexity sanity
check, not a release performance budget or cross-platform benchmark.

This is Windows local NTFS development coverage only. The API is
internal/test-gated, does not read snapshot file blobs for classification, and
does not provide a public CLI/SDK contract.

## M2-SLICE-005 Executed Coverage

`tests/workspace_diff.rs` proves the internal current-tree-to-head diff:

- a clean workspace returns no changes against its durable head;
- added, removed, modified, and file/directory type changes are classified;
- multiple changes omit unchanged paths and retain canonical byte ordering;
- repeated scans produce equal structured and serialized results;
- a 10,000-file current tree is classified without publishing a snapshot;
- configured secret bytes in the current tree fail closed before a diff is
  returned;
- corrupt reference manifests, missing heads, and inconsistent head identity
  fail closed;
- cold reopen preserves the same result; and
- a mutation between calls is explicitly treated as separate point-in-time
  observations rather than a watcher or lock guarantee.

The retained record is
[`m2-slice-005-workspace-diff-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-005-workspace-diff-windows-native-2026-09-01.json).
This is Windows local native / NTFS development evidence only, not M2 release
or cross-platform qualification. The current manifest is ephemeral; the
operation mutates neither CAS nor metadata.

## M2-SLICE-006 Executed Coverage

`tests/workspace_diff_reconciliation.rs` proves the lifecycle boundary around
the current-tree observation:

- a successful result records the durable observation revision, environment
  binding, and stable status;
- a changed workspace produces a non-empty diff, and snapshot publication
  advances the head/revision so the next diff is empty;
- a verified restore destination compares empty against its source snapshot,
  then reports a modification after a destination edit;
- repeated observations serialize identically and do not mutate head,
  revision, leases, operations, or events;
- a missing environment binding fails closed;
- a concurrent test-harness revision change during a 10,000-entry scan is
  rejected after the scan with `CONFLICT/UNSTABLE_OBSERVATION`;
- cold reopen preserves the observation result and the durable new head; and
- a legacy-compatible repository remains readable.

The retained record is
[`m2-slice-006-workspace-diff-reconciliation-windows-native-2026-09-01.json`](../../artifacts/m2-development/m2-slice-006-workspace-diff-reconciliation-windows-native-2026-09-01.json).
The run is `LOCAL_NATIVE_DEVELOPMENT` on Windows x86_64/NTFS and is not M2
release or cross-platform qualification. The concurrent revision mutation is
test-harness coordination evidence and is explicitly not a host-resource
failure claim.

## M2-SLICE-007 Executed Coverage

`tests/workspace_lifecycle_contract.rs` executes all 15 L1-L15 contract cases:

- create/open, logical close/archive, invalid and repeated transitions;
- `snapshot`/`restore`/`diff`/`status` capability negotiation and deterministic
  unsupported failure;
- explicit `TEST_DOUBLE` success, provider failure, interruption, and core
  persistence failure without phantom success;
- reconciling recovery transitions and deterministic operation/request retry;
- read-only status and current-tree diff behavior;
- a real SQLite v0.1 fixture opened and read without provider metadata;
- provider-context isolation from paths and provider-specific metadata; and
- generation selector and project event ordering unchanged across the provider
  boundary.

The test double is **SYNTHETIC** provider behavior. Repository-backed cases
are Windows local native development evidence on NTFS. The retained record is
[`m2-slice-007-workspace-lifecycle-contract-windows-native-2026-09-02.json`](../../artifacts/m2-development/m2-slice-007-workspace-lifecycle-contract-windows-native-2026-09-02.json).
This slice adds no non-local provider qualification, public API, SQLite schema,
or physical close/destroy guarantee. Full M1/M2 regression remained green;
existing host-resource and measurement-only tests marked `ignored` remain
outside the PASS count.

## M2-SLICE-008 Executed Coverage

`tests/workspace_lifecycle_integration.rs` executes eight durable integration
cases over a real repository and SQLite metadata store:

- create/open plus legal capture, snapshot, active, pause, resume, and close
  transitions with monotonic revisions;
- invalid transition, stale revision, and stale lease fail-closed behavior;
- exact retry recognition without a second revision increment;
- pre-commit rollback and post-commit durable-state retry behavior;
- interrupted `reconciling` transition, cold reopen, and explicit recovery;
- a synthetic provider failure that leaves durable core state unchanged;
- read-only status/diff and absence of a synthesized lifecycle event; and
- real SQLite v0.1 open/read/status/lifecycle-path compatibility behavior.

The production path reuses `MetadataStore::update_workspace`; no new schema,
operation table, lifecycle event type, or provider implementation was added.
The retained record is
[`m2-slice-008-workspace-lifecycle-integration-windows-native-2026-09-02.json`](../../artifacts/m2-development/m2-slice-008-workspace-lifecycle-integration-windows-native-2026-09-02.json).
The run is Windows local native / NTFS development evidence, not M2 release or
cross-platform qualification. Existing ignored host-resource and measurement
tests remain outside PASS counts.

## M2-SLICE-009 Executed Coverage

`tests/workspace_lifecycle_operation.rs` is a real durable integration suite
over the SQLite metadata store. It covers operation identity creation and
binding, successful lifecycle mutation, exact retry, changed operation
semantics, stale revision/lease, pre-commit rollback, post-commit interruption
and cold-reopen retry, provider failure isolation, read-only operation
exclusion, event taxonomy preservation, and raw v0.1 compatibility. The suite
passes 8/8 cases on Windows 11 / NTFS local development. Operation intent,
workspace update, terminal operation outcome, journal phase, and operation
events are verified as one transaction; no second ledger or schema is added.

## Unit

- Validate workspace/snapshot IDs, project binding, status transitions, and
  lease/revision preconditions.
- Validate status identity, head/snapshot/CAS integrity, lease activity,
  environment binding, changed semantics, and unresolved-operation state.
- Validate canonical manifest ordering, path portability, reserved names,
  traversal, invalid UTF-8, link/reparse rejection, and size/file limits.
- Validate snapshot metadata fields against manifest digest, CAS domain,
  workspace/project, environment, generation, and redaction identity.
- Validate deterministic snapshot-to-snapshot and current-tree-to-head diff
  output, including fail-closed head identity checks.
- Validate restore destination policy, temporary path safety, and typed errors.
- Validate operation/event payload shape and redaction before persistence.

## Integration

- Create/open a local workspace, acquire a lease, capture a snapshot, publish
  metadata/head, close, and cold-reopen with the same identity.
- Repeat an identical published snapshot after a post-commit outcome-unknown
  result and confirm one canonical digest, metadata row, event, and head.
- Change/add/delete files and verify workspace status; compare verified
  snapshots and the current workspace tree for deterministic path-level diff
  output.
- Restore to a new destination and verify bytes, manifest, and digest; reject
  an existing destination without altering it.
- Query workspace status before/after snapshot, after restore, after cold
  reopen, and after a lease expiry or unresolved operation.
- Verify snapshot/restore operation records and ordered events reference the
  same project, workspace, generation, and snapshot identity.
- Keep all M1 event, projection, migration, recovery, security, and artifact
  consistency tests as mandatory regression tests.

## Property

Use only the M2 corpus defined here; do not expand M1's corpus policy by
assumption. The initial normative properties are:

1. Canonical manifest identity is invariant under directory enumeration order.
2. Valid portable paths round-trip through manifest read/materialization.
3. Invalid paths and unsupported entries never advance workspace head/revision.
4. Repeated snapshot of unchanged content converges to one digest and metadata
   result.
5. Snapshot metadata never points to a missing, wrong-domain, or wrong-identity
   CAS object.
6. Restore of a verified snapshot is deterministic and a retry does not
   overwrite an existing destination.
7. A failed publication leaves either the complete pre-state or a verified,
   explicitly discoverable post-state; it never creates mixed generation state.

Each retained run records seed, requested/executed cases, command, toolchain,
platform/filesystem, exit code, and failure/shrink output. A default local case
count is not a normative release count until the corpus policy is accepted for
M2.

## Migration

- Open a raw `v0.1.0` fixture read-only and prove no M2 DDL or source mutation.
- Migrate to a target generation with additive snapshot/workspace metadata,
  verify generation and migration identities, and cold-reopen.
- Interrupt before and after selector publication; assert old-or-new visibility
  and no mixed workspace/snapshot generation.
- Reject wrong-format, wrong-generation, wrong-project, and incompatible
  redaction metadata before mutation.

## Crash Recovery

- Snapshot publication: before blob, after blob, before manifest, after
  manifest, before metadata commit, after metadata commit.
- Workspace-head update: pre-commit, post-commit, and process termination;
  cold-reopen must classify the head and snapshot metadata consistently.
- Restore materialization: file write, file sync, tree sync, rename, parent
  sync, and temporary cleanup; inspect destination and orphan directories after
  reopen.
- Confirm no fabricated successful operation when recording or provider result
  is unknown. Verify retry/idempotency and explicit reconciliation paths.

## Fault Matrix

The first matrix is limited to the existing M2 gate scenarios:

| Fault | Expected invariant |
| --- | --- |
| CAS permission/resource/short write | No readable partial object; head/revision unchanged. |
| File changes during read | Snapshot fails closed; no misleading manifest publication. |
| Metadata commit before/after | Cold reopen observes complete pre-state or complete post-state. |
| Materialization write/sync/rename | Existing destination untouched; temporary cleanup is bounded. |
| Parent sync after rename | Destination retained and result marked outcome-unknown. |
| Abandoned temporary directory | Only matching safe temporary paths are removed or quarantined. |
| Registered secret in file/path/metadata | Rejected/redacted before persistence; no head advance. |
| Symlink/reparse or unsafe path | Integrity failure before traversal/materialization. |

Failpoint evidence is `SYNTHETIC`; host ACL, quota, and filesystem evidence
must be labeled by actual platform and filesystem and cannot be inferred from a
different row.

## Performance

Record repeatable measurements for repository open, workspace status, snapshot
walk, current-tree diff, blob publication, manifest publication, head update,
snapshot diff, and restore materialization at at least documented
small/medium/large tree sizes.
Capture samples, p95/p99 where meaningful, bytes/files, filesystem, toolchain,
and command. These are M2 measurements only; do not invent a new budget or
accept ADR-0015 in this plan.

## Compatibility

- M1 Event Envelope, Projection Contract, project sequence/cursor, operation
  ledger, generation identity, selector atomicity, redaction, and recovery
  tests must remain green.
- Existing v0.1 readers must not be silently required to understand additive
  M2 tables; migration fixtures must prove source preservation.
- Unknown additive event fields remain opaque to existing readers; incompatible
  schema changes require a new ADR and compatibility fixture.

## Platform Evidence

The M2 evidence matrix must distinguish:

- Windows local native / NTFS;
- Linux native environment / declared filesystem;
- macOS GitHub-hosted runner qualification / filesystem as actually reported.

Linux VM evidence is native Linux evidence for that VM, not a physical server;
GitHub-hosted macOS is not a physical-Mac or APFS claim. Unavailable or skipped
commands remain `NOT_EXECUTED`, never `PASS`.

## Exit Criteria

M2 remains internal until all required tests, fault schedules, cold-reopen
results, reconciliation decisions, measured costs, migration fixtures, and
updated security/reliability/architecture documents are retained. The final
M2 gate must state which evidence is native, hosted, synthetic, or unavailable
and must preserve an M1 regression PASS.
