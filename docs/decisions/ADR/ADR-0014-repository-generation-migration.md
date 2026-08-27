# ADR-0014: Cross-File Repository-Generation Migration

- Status: Accepted for M1 implementation evidence
- Date: 2026-08-20

## Context

The v0.1 repository stores authoritative metadata in `.pong/metadata.sqlite`.
Changing its storage schema or driver in place would make a crash expose a
partially transformed database, and a row such as `active_generation` inside
that same file would not protect readers that still open the legacy path. M1
requires historical events, refs, idempotency records, operation journal rows,
and opaque extensions to survive a restartable migration without mixed
visibility.

## Decision

Pong migrates metadata across SQLite files. The legacy database remains intact
and `.pong/generations/<generation-id>/metadata.sqlite` is built with SQLite's
Online Backup API. A generation manifest records the repository/schema/storage
identity, source marker digest, migration-time target metadata digest and size,
and redaction profile identity. The target also stores the generation ID and
migration ID in SQLite metadata. Startup compares those durable identities with
the selected manifest, so replacing an active generation with another
format-valid database fails closed. The target is checkpointed, integrity-checked,
foreign-key-checked, redaction-checked, synced, and then made visible by an
atomic replacement of `.pong/repository.json`.

The root selector is the only visibility commit point. Before replacement,
startup uses the legacy marker and ignores prepared generations. After
replacement, startup follows the selector, validates only that generation, and
may idempotently complete the migration journal; it never silently rolls back
to the legacy file. A v0.1 reader sees the v0.2 marker identity and returns
`UNSUPPORTED` before opening or mutating the old metadata path.

Migration is explicitly offline. Current Pong handles hold a shared lock and
the migration operation takes an exclusive lock. Because older binaries do not
know this lock, operators must close them before migration. IDs used in paths
are safe single components; symlinks, junctions, and Windows reparse points
are rejected rather than followed.

On POSIX, selector replacement is fsynced temporary-file plus rename-overwrite
and parent-directory sync. On Windows, the implementation uses
`MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` and performs the corresponding
directory synchronization. Directory-handle access failures are classified
through the protected repository boundary (`PERMISSION_DENIED` for a final ACL
denial), while rename/share races retain their generic I/O classification and
retry policy. A failure after replacement is outcome-unknown for bookkeeping,
not permission to delete the new selector or restore the old one.

## Consequences

### Positive

- Old and new visibility have one durable namespace commit point.
- SQLite WAL sidecars are included through the supported backup API rather than
  being copied as an inconsistent main-file snapshot.
- Repeated migration is bound to a plan digest and is idempotent after selector
  publication.
- Legacy bytes remain available for rollback tooling or explicit operator
  recovery without being used implicitly by a new reader.

### Costs and boundaries

- Migration is offline and requires enough space for a second SQLite file.
- The manifest metadata digest is a migration-time backup attestation; the
  active database is mutable, so startup uses SQLite integrity/FK/profile checks
  and the in-database generation/migration identity instead of comparing live
  bytes to that historical digest. This also detects a replacement by a
  different but otherwise format-valid SQLite file.
- The current evidence is Windows-local. Real host quota/permission schedules,
  supported-platform matrices, property corpus runs, and performance budgets
  remain open M1 work.

## Rejected alternatives

- **`active_generation` row in the legacy database:** does not protect old
  readers that open the old path and can expose a mixed cross-file state.
- **Copying `metadata.sqlite` directly:** ignores WAL/SHM/journal state and is
  not a consistent SQLite backup contract.
- **Delete then rename selector:** creates a namespace gap and is not atomic.
- **Automatic rollback after selector replacement:** can make two generations
  appear authoritative after a crash; the selector must win.

## References

- [`STORAGE_ARCHITECTURE.md`](../../architecture/STORAGE_ARCHITECTURE.md)
- [`VERSIONING_PROTOCOL.md`](../../protocol/VERSIONING_PROTOCOL.md)
- [`RECOVERY.md`](../../reliability/RECOVERY.md)
- [`M1_DURABLE_PRIMITIVES_GATE.md`](../../development/M1_DURABLE_PRIMITIVES_GATE.md)
- [`ADR-0013-core-runtime-and-metadata-binding.md`](ADR-0013-core-runtime-and-metadata-binding.md)
