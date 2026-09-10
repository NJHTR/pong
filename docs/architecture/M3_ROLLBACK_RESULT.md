# M3 Rollback Result Semantics

**Status:** `IMPLEMENTED / INTERNAL / TEST-GATED`
**Slice:** M3-SLICE-002D - Rollback Result / Materialization Core
**Contract origin:** M3-SLICE-002C - Rollback Result Semantics Final Contract
**Implementation:** Local Workspace materialization, guarded dual-head
publication, prepared-intent retry, crash-boundary recovery, and legacy
compatibility are implemented and covered by real SQLite/filesystem tests.
No provider integration, public API, DDL change, release tag, or push is
included in this slice.

## Existing Invariants

The contract is additive to the existing M1/M2/M3 authorities:

- `Workspace.head` is the digest of the Snapshot content currently represented
  by the physical Workspace tree.
- `Workspace.version_head_id` is the current logical Version selection and is
  independent from `Workspace.head`.
- Snapshot identity and bytes are immutable.
- Version identity, `parent_version_id`, Snapshot binding, and creation
  Operation identity are immutable.
- Execution identity and parent Execution lineage are immutable.
- Workspace writes require the existing lease epoch and revision CAS.
- Rollback never deletes history, rewrites a Version, or behaves as
  `git reset --hard`.

## Recommended Model

`ROLLBACK = HISTORY-PRESERVING RECOVERY`.

For a graph `V100 -> V101 -> V102 -> V103`, a rollback from the current
Workspace state to `V100` restores the Workspace to the Snapshot referenced by
`V100`. `V101`, `V102`, and `V103` remain durable and unchanged. Rollback does
not create a new Version. A later Resume/Continue Execution may create a new
Snapshot and Version, for example `V104` with `parent_version_id = V100`.

Rollback and Resume are separate operations:

```text
Rollback(W1, V100) -> W1 contains S100
Resume from V100   -> new Execution E4
E4 changes tree    -> new Snapshot/Version V104
```

## Rollback

The target is an immutable Version or Checkpoint resolving to one healthy
Version. The target Version remains unchanged. A Checkpoint remains an
immutable reference and is not moved or copied.

Rollback itself produces a completed recovery result, not a Version result.
Its `result_version_id` is therefore `NULL`; the result Version is created only
by a subsequent new Execution.

## Workspace Head

After successful materialization and metadata publication:

```text
physical Workspace tree = target Snapshot content
Workspace.head           = target Snapshot root digest
```

The state `physical tree = S100` with `Workspace.head = S103` is forbidden.
The existing Snapshot Head meaning is preserved; no new head field is added.

## Version Head

After successful rollback, `Workspace.version_head_id` is set to the target
Version using the existing lease/revision-guarded Version Head path. If the
current selection was `V103` and the target is `V100`, the result is:

```text
Workspace.version_head_id = V100
```

`V103` remains durable and is not rewritten. A later `V104` created by Resume
may become the Version Head through the existing explicit selector path.

## Snapshot

The target Snapshot is reused as immutable content. Rollback does not create,
modify, or delete Snapshot bytes and does not duplicate a Version manifest.

## Version

Rollback does not create a Version. Resume creates the next Version from the
restored state under the existing M2 Version creation contract. The new Version
must use an explicit parent, normally the rollback target Version, and must not
overwrite `V103`.

## Version Graph

Historical lineage is preserved:

```text
V100
|- V101
|- V102
|- V103
`- V104   (created later by Resume, parent V100)
```

No parent edge, identity, or historical row is rewritten or deleted.

## Rollback Record

The logical result requires these values, whether represented by existing
columns or additive proposal columns:

| Field | Meaning |
| --- | --- |
| `rollback_id` | Immutable request identity |
| `task_id`, `execution_id` | Authorized scope |
| `source_version_id` | Version represented before rollback |
| `target_version_id` | Historical Version being restored |
| `checkpoint_id` | Optional immutable target anchor |
| `previous_workspace_head` | Snapshot digest before materialization |
| `result_workspace_head` | Target Snapshot digest after materialization |
| `previous_version_head` | Version selection before rollback |
| `result_version_head` | Target Version selection after rollback |
| `status` | Old/new/unknown recovery classification |
| `reason` | Redacted recovery reason |
| `created_at`, `completed_at` | Durable timing |
| `result_version_id` | `NULL` for rollback; populated only by a later Version-producing operation if the relation is extended |

The record must answer who restored which Workspace from which state to which
state. It is immutable after a completed result except for an explicitly
defined recovery transition from `unknown`.

## Checkpoint

`Checkpoint C1 -> V100` remains unchanged. `Rollback(C1)` resolves to `V100`,
restores its Snapshot, and leaves the Checkpoint immutable.

## Resume

Resume creates a new Execution after rollback. It never revives or rewrites the
old Execution. The new Execution uses the rollback target Version as its
explicit base, and a later Snapshot/Version publication represents its new
work.

## Handoff

Handoff remains a separate durable responsibility relation. The Codex -> Cursor
scenario is:

```text
E1/Codex -> quota exhausted -> Handoff -> E2/Cursor
E2 error -> Rollback(C1) -> Workspace restored
Resume -> E3 -> later new Version
```

Handoff does not transfer a Workspace lease implicitly.

## Parallel Isolation

Execution-scoped rollback is Workspace-local. Rolling back `E1/W1` cannot
change `E2/W2`, `E3/W3`, their Version Heads, or their historical Versions.
Task-level rollback requires a separate explicit scope and transaction
contract; it is not inferred from an Execution rollback.

## Crash Recovery

Filesystem and SQLite are not one ACID transaction. The implementation must use
an old-or-new protocol:

1. prepare and durably identify the rollback request and prior heads;
2. acquire and retain the Workspace lease/revision authority;
3. materialize the target Snapshot into a temporary tree;
4. verify CAS, manifest, and materialized content;
5. atomically replace the Workspace tree;
6. publish `Workspace.head`, Version Head, and the completed RollbackRecord in
   the existing guarded metadata transaction;
7. reconcile on reopen when acknowledgement or a boundary is uncertain.

Cold reopen may expose only `OLD COMPLETE STATE` or `NEW COMPLETE STATE`.
Partial tree, mismatched head, dangling Version Head, or completed metadata
without verified materialization is forbidden. An uncertain boundary remains
explicitly `unknown` until reconciliation.

## Idempotency

The same rollback request identity, target, scope, actor, lease epoch, and
revision converges to the same durable RollbackRecord. A completed retry does
not create another rollback or Version. A changed target, scope, actor, or
request digest fails deterministically.

## Failure Boundary

Missing/corrupt Snapshot, missing CAS, invalid manifest, permission denial,
invalid Workspace, stale lease, stale revision, materialization failure, and
metadata failure all fail closed. No failure may publish `completed` while the
physical result is unverified. Pre-commit failure leaves the old complete
state; post-commit uncertainty is reconciled as old or new.

## Compatibility

The contract does not change M1 v0.1.0, ADR-0016, Snapshot meaning, existing
Restore semantics, `Workspace.head`, Version identity/parentage, Version Head
authority, Operation identity, Execution identity, or v0.1 reader behavior.
Legacy repositories do not receive synthetic RollbackRecords or baselines.

## Schema Proposal

No DDL or schema change is executed in 002D. The existing
`rollback_records` relation remains the durable identity boundary. If a future
implementation needs the explicit
before/after head and completion fields listed above, they are additive schema
proposal only and require a separately reviewed migration. No existing column
meaning may be repurposed.

## Test Contract

`tests/rollback_result_contract.rs` retains RR1-RR22 as explicitly ignored
`NOT_IMPLEMENTED_CONTRACT_TEST` index cases. Real runtime coverage is in
`tests/rollback_result.rs` with 30 passed, 0 failed, and 0 ignored cases for
target resolution, both heads, history, immutability, no Version creation,
Resume, idempotency, crash-boundary recovery, physical/metadata consistency,
parallel isolation, Codex -> Cursor -> Rollback -> Resume, failure modes,
lease, revision, phantom-success prevention, and legacy compatibility.

## Open Decisions

The rollback result semantics are closed by this contract. These unrelated
policies remain open and are not silently decided here:

- Task versus Execution checkpoint scope;
- Task baseline storage;
- selective/component or cross-workspace rollback ownership;
- Handoff authorization/controller policy;
- checkpoint trigger, retention, and deletion policy;
- simultaneous Handoff relay policy; and
- event taxonomy and retention beyond existing Operation linkage.

## Implementation Status

`WorkspaceManager::rollback_local` records a prepared intent, validates the
target Version/Checkpoint and local CAS manifest, materializes and verifies a
temporary sibling tree, replaces the existing Workspace tree, and publishes
`Workspace.head`, `Workspace.version_head_id`, the guarded revision increment,
and rollback completion in one SQLite transaction. Completion failpoints and
reopen retry converge deterministically to a complete old-or-new state.

`result_version_id` remains `NULL`: rollback does not create a Version. A
subsequent Resume creates a new Execution and later Version. Historical Version
rows, parent edges, Snapshot bytes, Operations, Executions, and Checkpoints are
not rewritten.

This is Windows local-native development evidence only. It is not M1 release
evidence and does not claim non-local provider support.

## Risks

The remaining implementation risk is the non-ACID boundary between physical
replacement and SQLite metadata. The implementation retains a durable
prepared marker and verifies both authorities on retry/reopen. A second risk
is accidentally treating Version Head as Snapshot Head; the implementation
records and validates both values independently. Task-scoped rollback,
cross-workspace rollback, provider adapters, and public API policy remain
outside this slice.
