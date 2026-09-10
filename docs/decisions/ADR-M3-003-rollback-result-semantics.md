# ADR-M3-003: Rollback Result Semantics

- Status: `Implemented / Internal / TEST-GATED`
- Date: 2026-09-08
- Scope: M3-SLICE-002C contract closure
- Implementation slice: M3-SLICE-002D local materialization core
- Implementation: M3-SLICE-002D local materialization core; no DDL, provider
  integration, release tag, or push

## Context

M3-SLICE-002B implemented durable Handoff, Checkpoint, Rollback-record, and
Resume metadata. M3-SLICE-002C closed the rollback materialization result,
Workspace Head update, and Version Head selection semantics without changing
the historical ADR-M3-002 text or any M1/M2 authority. M3-SLICE-002D now
implements that closed local materialization path.

## Decision

Adopt `ROLLBACK = HISTORY-PRESERVING RECOVERY`:

1. Rollback restores the authorized Workspace physical tree to the target
   Version's Snapshot content.
2. `Workspace.head` becomes the target Snapshot root digest.
3. `Workspace.version_head_id` becomes the target Version through the existing
   lease/revision-guarded selector.
4. Rollback itself creates no Version and has `result_version_id = NULL`.
5. A later Resume creates a new Execution; its subsequent Snapshot/Version is
   the new state, normally with the rollback target as explicit parent.
6. All historical Versions, Snapshots, Operations, Executions, Checkpoints,
   Version identities, and parent edges remain unchanged.
7. Checkpoints remain immutable references.
8. Execution-scoped rollback is Workspace-local by default; Handoff never
   transfers a lease.
9. Crash handling is old-or-new with prepare, temporary materialization,
   verification, guarded metadata publication, and reopen reconciliation.
10. Exact retry is deterministic and never creates a second rollback or a
    second Version.

## Existing Invariants Preserved

`Workspace.head` remains a Snapshot content head, Version Head remains an
independent logical selection, and the M2 Snapshot/Restore/Version contracts,
M1 v0.1.0 meaning, ADR-0016, Operation identity, and Execution identity are
unchanged.

## Recommended Model

```text
V100 -> V101 -> V102 -> V103
                         |
                 Rollback to V100
                         |
Workspace = S100, Workspace.head = S100 digest,
Version Head = V100, result_version_id = NULL
                         |
                 Resume -> E4 -> V104(parent V100)
```

The historical `V101`-`V103` rows remain intact; `V104` is not created by the
Rollback operation.

## RollbackRecord Contract

The durable result must retain source and target Version identity, optional
Checkpoint, previous/result Workspace Head, previous/result Version Head,
status, redacted reason, creation/completion timestamps, and request identity.
Existing columns may be reused only with their current meaning. Any missing
before/after fields are additive schema proposal, not an implicit DDL change.

## Atomicity and Recovery

Physical filesystem replacement and SQLite metadata publication cannot be a
single host ACID transaction. Implementations must use a durable prepare and
reconcile boundary. A completed record requires verified target materialization
and matching heads. Reopen must classify uncertainty explicitly; phantom
success, partial tree, stale Workspace Head, and dangling Version Head are
forbidden.

## Failure and Idempotency

Missing or corrupt content, CAS, manifest, Workspace, lease, or revision, plus
materialization and metadata failures, fail closed. Exact request retry returns
the existing durable result. Changed target, actor, scope, or digest is a
deterministic conflict.

## Compatibility

No legacy schema meaning changes and no synthetic relation is generated for a
v0.1 repository. Version graph, Snapshot bytes, Workspace Head, Version Head,
Operation, and Execution authorities remain as previously defined.

## Non-Goals

This ADR does not add SQLite schema or DDL, providers, Branch, Merge, Rebase,
Candidate, Approval, Agent State, Memory, Skill, Automation, CLI, SDK, UI, or
release workflow behavior. The local materialization implementation described
by this decision is limited to M3-SLICE-002D and is not a public provider API.

## Closed Decisions

- Rollback preserves all history.
- Rollback does not rewrite Version identity or parentage.
- Rollback restores the target Snapshot into the authorized Workspace.
- `Workspace.head` becomes the target Snapshot digest.
- Version Head becomes the target Version.
- Rollback creates no Version; `result_version_id` is `NULL`.
- Resume creates the new Execution and later Version.
- Checkpoints do not move or mutate.
- Parallel rollback is isolated to the target Workspace by default.
- Handoff does not transfer a lease.
- Crash recovery is old-or-new with deterministic reconciliation.
- Exact retry is deterministic.

## Open Decisions

- Task versus Execution checkpoint scope;
- Task baseline storage;
- selective/component and cross-workspace rollback ownership;
- Handoff authorization/controller policy;
- checkpoint trigger and retention/deletion policy;
- simultaneous Handoff relay policy; and
- event taxonomy beyond existing Operation linkage.

## Test Contract

`tests/rollback_result_contract.rs` defines RR1-RR22 and remains an ignored
contract index. The implemented local path is covered by
`tests/rollback_result.rs`: 30 passed, 0 failed, 0 ignored.

## Consequences

Rollback becomes a precise state restoration boundary while Resume remains the
Version-producing boundary. Implementers must coordinate physical replacement,
Workspace Head, Version Head, and RollbackRecord recovery without weakening
the existing lease/revision or immutable-history rules.

## Implementation Status

`M3-SLICE-002D = PASS / INTERNAL / TEST-GATED`.

`WorkspaceManager::rollback_local` prepares a durable intent, materializes and
verifies the target Snapshot, replaces the local Workspace tree, and publishes
both heads and completion with the existing revision/lease guard. Exact retry,
metadata failure boundaries, cold reopen, CAS integrity failures, and legacy
compatibility are runtime-tested. The ADR remains internal/test-gated; it does
not authorize public API or release acceptance.
