# Recovery

**Implementation status:** M1 repository/journal recovery and the bounded M3
operation ledger are exercised internally. The M2 local workspace slice does
not yet implement provider crash reconciliation or a snapshot-intent journal;
its materialization and CAS failure requirements are tracked in
[`M2_WORKSPACE_SNAPSHOT_GATE.md`](../development/M2_WORKSPACE_SNAPSHOT_GATE.md).
M3 records durable operation intents and terminal outcomes but does not infer
or reconcile external effects; see
[`M3_OPERATION_LEDGER_GATE.md`](../development/M3_OPERATION_LEDGER_GATE.md).

## Recovery goals

After agent, process, Pong, machine, or disk failure, Pong must preserve committed history, identify uncertain effects, and restore a usable workspace without fabricating events. Recovery is local-first; remote replication, when configured, is an additional copy rather than the source of truth.

## Write-ahead journal

Before invoking a mutating tool, the runtime appends an intent with request ID, operation digest, resource preconditions, and policy decision. It fsyncs the intent, executes the tool, then appends outcome and state delta. Publication to the event index and ref transaction follows. Intents without outcomes are recovered as `unknown`.

## Startup procedure

1. Verify repository format and journal integrity.
2. Replay complete journal records into the event index idempotently.
3. Mark orphan intents `unknown`; run tool/workspace reconciliation where possible.
4. Verify refs, snapshots, and reachable blobs; quarantine corrupt objects.
5. Rebuild caches and expose degraded status until checks pass.

## Generation migration recovery

Migration is explicitly offline. Current Pong processes take a repository
lock, and operators must close older binaries that do not understand that lock.
The legacy selector remains authoritative while a target generation is being
allocated, backed up, checkpointed, and verified. Temporary or unreferenced
generation directories are cleaned or quarantined and are never adopted by
startup based on directory order.

`repository.json` is the migration commit point. Before its atomic replacement,
a crash or resource failure leaves the old generation visible. After its
replacement, startup trusts the selector, verifies the selected manifest and
SQLite database, and idempotently completes the migration journal if the
process stopped before journal finalization. It does not silently roll back to
the legacy file. A missing/damaged selector, manifest, target database, or
inconsistent migration journal is `RECOVERY_REQUIRED` or `INTEGRITY_ERROR`.
For an active generation, startup also checks that the SQLite
generation/migration identity matches the selected manifest; a different
format-valid database is therefore rejected as an integrity failure rather
than treated as a recoverable alternate generation.

## Reconciliation

Filesystem reconciliation compares manifests and hashes, attributing unobserved changes to a synthetic `reconciled` operation. Process/network/database reconciliation uses provider logs or explicit operator confirmation. No reconciliation claims reversibility it cannot demonstrate.

## Restore and rollback

Restore selects a checkpoint/snapshot and target dimensions. It writes a new restore operation, creates a safety snapshot, applies changes atomically per resource where possible, and verifies hashes. External effects and secret stores are excluded unless an explicit compensating action exists.

## Backup and retention

Back up refs, metadata, journals, and reachable blobs as a consistent generation. Test restores regularly. Retention policies may prune unreachable artifacts and raw payloads, but audit/event summaries needed for provenance are retained according to policy.
