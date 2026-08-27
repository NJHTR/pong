# Storage Architecture

## v0.x decision

Pong is local-first. A project keeps its control data under `.pong` and must remain usable offline. Storage interfaces separate metadata, events, immutable objects, and artifacts so each can later be replicated or replaced.

## Stores

| Store | v0.x implementation | Data |
| --- | --- | --- |
| Metadata store | SQLite in WAL mode | projects, agents, workspaces, tasks, refs, leases, policies |
| Event store | SQLite append-only tables plus export stream | operation and lifecycle events |
| Object store | Filesystem CAS | snapshot manifests, trees, commit objects |
| Artifact store | Filesystem blobs by digest | large outputs and binary media |
| Journal | SQLite WAL and operation intent rows | crash recovery and reconciliation |

SQLite is chosen for transactional local metadata and queryability. A custom database or PostgreSQL is deferred until multi-process or remote coordination requirements are demonstrated. Git object storage is conceptually similar but cannot represent execution evidence and redaction metadata alone.

## `.pong` layout

```text
.pong/
  config
  metadata.sqlite
  objects/       # immutable CAS objects
  refs/          # human-readable ref cache, SQLite remains authoritative
  events/        # append/export segments
  artifacts/     # digest-addressed large outputs
  snapshots/    # provider manifests and recovery metadata
  agents/        # local identity and heartbeat cache
  workspaces/    # provider bindings, not the workspace data itself
  environments/  # redacted environment revisions
  locks/
```

The project working files remain outside `.pong`; a workspace provider maps logical ids to physical paths, containers, or remote machines.

In the current M2 internal slice, logical workspace rows, lease epochs, and
canonical environment facts live in the active SQLite generation. Local file
bytes and tree manifests use the existing typed filesystem CAS; workspace
materializations remain outside `.pong`. These new tables and object domains
are not a released repository-format promise. Their schema-evolution and
old-reader behavior must be covered before the M2 gate can pass.

The v0.1 layout is a legacy source. An offline migration keeps that source and
builds a new metadata generation under a versioned selector:

```text
.pong/
  repository.json                 # the only active-generation commit point
  metadata.sqlite                 # legacy v0.1 source; retained after migration
  generations/
    <generation-id>/
      metadata.sqlite
      generation.json              # compatibility and backup attestation
  migrations/<migration-id>.json   # restartable migration journal
  locks/repository.lock            # process-level shared/exclusive fence
```

`Repository::open` follows `repository.json` and never treats the legacy
database as authoritative after a selector is visible. A v0.1 reader sees the
incompatible marker identity and returns `UNSUPPORTED` before opening or
mutating the old path. Migration uses SQLite's Online Backup API; it does not
copy a main database while ignoring WAL/SHM/journal sidecars. The target is
integrity-checked, foreign-key-checked, redaction-profile-checked, and fully
synced before an atomic selector replacement. Before replacement, recovery
keeps the legacy selector visible; after replacement, the selector wins and
startup only finalizes bookkeeping, never rolls back to the old generation.

The manifest's metadata digest and size attest to the verified backup image at
the migration boundary. The target SQLite metadata also stores the generation
and migration identities and startup requires them to match the selected
manifest. Since the active SQLite file remains mutable, normal startup
validates its current SQLite integrity, foreign keys, format, redaction profile,
and durable identity rather than comparing live bytes to that historical
snapshot.

## Durability and retention

Writes use intent, append, and commit phases. Immutable objects are written before refs move. Garbage collection is mark-and-sweep from refs, checkpoints, retained events, and pinned artifacts. Retention policy is explicit because deleting evidence can break replay or audit requirements.

The metadata store and CAS share the repository redaction profile. Structured metadata is deterministically redacted before canonicalization; exact CAS bytes are rejected when they contain a configured secret because rewriting an object would change its content identity. Startup scans SQLite sidecars and CAS trees fail-closed before reporting a repository healthy.

## Remote evolution

Every store has a repository interface. Remote replication may upload immutable objects and event segments first, then transactionally advance refs with expected-old-value checks. Conflict resolution remains a domain concern, not an object-store side effect.
