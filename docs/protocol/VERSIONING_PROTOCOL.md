# Versioning Protocol

## Version domains

Pong distinguishes protocol, schema, object, storage, and CLI versions. They evolve independently and are recorded in every project manifest and commit metadata. A client must not assume that equal numbers imply compatibility across domains.

## Semantic rules

Protocol and public API versions use SemVer once stable; pre-1.0 releases treat minor changes as potentially incompatible unless explicitly marked additive. Object schemas use a numeric `schema_version` and migration functions. Storage format changes are gated by a repository format number and an offline upgrade operation. The v0.2 generation format uses a root selector rather than an `active_generation` row inside the legacy SQLite file.

## Compatibility window

v0.x readers support the current and previous minor protocol versions. Writers emit the negotiated version and retain required fields for the window. Unsupported major versions fail with `UNSUPPORTED` before mutation. Feature flags are capability-negotiated rather than inferred from version strings.

## Evolution of objects

Fields are classified as required, optional, derived, or opaque extension. Required fields cannot be removed without a major schema version. Enum values are open by default; unknown values remain opaque. IDs and hashes are immutable. A renamed field is represented by dual read/write during a deprecation window and a migration note.

## History and migrations

Historical events, operations, snapshots, and commits are immutable. Migrations create a new metadata view or a new repository generation; they never rewrite event payloads in place. Every migration has a preflight, backup/rollback plan, integrity verification, and an ADR. Content-addressed blobs remain valid across metadata migrations.

Repository-generation migration is a cross-file operation. The legacy
`.pong/metadata.sqlite` remains intact while SQLite Online Backup builds
`.pong/generations/<id>/metadata.sqlite`. A generation manifest records the
source marker digest, target metadata backup digest/size, format/schema identity,
storage driver, and redaction profile. The target SQLite metadata also carries
the generation ID and migration ID, which must match the selected manifest;
this prevents a different but format-valid database from being adopted. The manifest and target are verified
before `.pong/repository.json` is atomically replaced. That selector replacement
is the sole visibility commit point: an interruption before it is old-only; an
interruption after it is new-only and may only complete the migration journal.
An active generation's live database may subsequently change, so its manifest
metadata digest is a migration-time attestation rather than a perpetual byte
hash.

Migration IDs and generation IDs are restricted to safe single path components.
Readers reject missing or damaged selectors, manifests, journals, and active
generations fail-closed with `UNSUPPORTED`, `INTEGRITY_ERROR`, or
`RECOVERY_REQUIRED`; they never guess another generation.

## Branch and commit compatibility

Commit objects declare object/schema versions and redaction profile. A branch may contain commits from older schemas if the reader can project them. Merge requires compatible projections; otherwise it fails with an explicit migration requirement. Replay is allowed only when operation schema and runtime capability are compatible.

## Deprecation policy

Deprecated commands and fields are documented, emit machine-readable warnings, and remain available for at least one minor release unless security requires removal. Breaking security fixes include a migration or quarantine path. Release notes list behavior changes, not only code changes.
