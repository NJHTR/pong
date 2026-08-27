# ADR-0013: Rust Core Runtime and SQLite Metadata Binding

- Status: Accepted with M1 implementation gate
- Date: 2026-08-19

## Context

Pong has completed the architecture and PoC phase but has not selected an
implementation language or a concrete SQLite binding. The choice affects the
durability boundary, crash testing, distribution, CLI behavior, SDK integration,
and the cost of replacing a driver after repositories exist.

The existing storage decision remains valid: v0.x uses a local SQLite metadata
and event store, a filesystem CAS for immutable objects, and a separate artifact
store. The SQLite file is control-plane state, not an incidental cache. A driver
must therefore expose predictable transactions, WAL behavior, integrity errors,
busy/lock handling, and recovery evidence.

The current PoCs use Node.js, but they are architecture models rather than an
implementation commitment. The public API design already includes a Python SDK,
and the wire protocol intentionally does not prescribe a language or transport.

## Problem

Which language/runtime should own Pong Core v0.x, and which SQLite binding should
implement the metadata store, while preserving the protocol and repository
format for future SDKs, providers, and replication?

## Requirements

The decision must support:

1. Explicit WAL and fsync/sync semantics rather than best-effort durability.
2. Deterministic crash, corruption, lock, quota, and migration tests.
3. Windows, macOS, and Linux local repositories, including common x64 and ARM64
   release targets where the M1 support matrix accepts them.
4. A small, reviewable dependency surface and a reproducible release artifact.
5. A stable local protocol so Python, Node.js, and future SDKs do not access the
   database directly.
6. A driver boundary that allows a future binding or remote store without
   changing domain semantics.

## Options considered

### TypeScript/Node.js with built-in `node:sqlite`

This has a good fit with JavaScript-based agent runtimes and avoids an additional
native package. The synchronous API is useful for transaction boundaries.

However, the API and availability are tied to a particular Node release line and
have not had the same long-term compatibility history as the Python standard
library or the SQLite C API. A repository would also inherit Node runtime
installation and upgrade policy. The built-in module does not remove the need to
test platform-specific WAL, locking, and filesystem durability.

### TypeScript/Node.js with an external SQLite package

`better-sqlite3` provides a mature synchronous API and is a viable adapter
candidate. Other callback-oriented or asynchronous packages add avoidable
ordering and shutdown complexity for a local durable core.

The cost is a native-addon ABI and prebuild matrix, compiler/toolchain failures
on unsupported targets, and an external security/maintenance dependency. The
binding is not a stable Pong contract and cannot be allowed to leak through the
domain layer.

### Python with the standard-library `sqlite3` module

Python has a stable, widely understood DB-API and no third-party SQLite package
is required. It aligns naturally with the proposed Python SDK and has strong
test tooling.

The Python executable and its bundled/system SQLite version still vary by
platform. Distribution as a self-contained CLI is less uniform than a native
binary, and the Core would share a process/runtime with fewer agent integrations
than a protocol service. The GIL is not a correctness problem for v0.x, but it
does not provide a reason to make Python the storage authority when the protocol
already separates clients from Core.

### Rust Core with `rusqlite`

Rust provides memory safety for the long-lived repository process, explicit
filesystem synchronization APIs, deterministic synchronous transaction code, and
single-binary CLI/service distribution. `rusqlite` is a maintained, direct
SQLite binding with transaction and error types suitable for a narrow adapter.
Using its bundled SQLite build pins engine features and avoids depending on an
unknown system SQLite installation.

The tradeoffs are a larger contributor/toolchain requirement, longer builds, and
the need to publish prebuilt binaries for supported platforms. These costs are
paid at the Core boundary and do not burden Python or Node agent authors because
they use the local protocol.

### Other choices

PostgreSQL, RocksDB, a custom WAL, a system `sqlite3` command, or a Go/Java
implementation would either violate the local-first scope, duplicate storage
semantics that SQLite already supplies, or introduce another SDK/distribution
surface without solving a current M1 requirement. They remain possible future
drivers only after a new scale or deployment requirement is demonstrated.

## Decision

### Core language and runtime

Pong Core v0.x is implemented in **Rust** as a library plus a local executable.
The executable owns repository startup, recovery, metadata transactions, event
projection, and the local protocol/CLI boundary. The implementation uses a
stable Rust toolchain pinned by the implementation repository; the exact toolchain
and minimum supported Rust version are release metadata, not implicit developer
defaults.

Rust is a Core implementation choice, not a public API choice. Python and
TypeScript/Node.js SDKs, framework adapters, and agent runtimes communicate over
the versioned Pong protocol and must not open or mutate `.pong/metadata.sqlite`
directly. The existing Python SDK design remains valid.

### Metadata binding

The v0.x metadata and event store is **SQLite through a thin Rust adapter using
`rusqlite` with a pinned/bundled SQLite engine**. No domain or protocol module may
depend on `rusqlite` types or issue ad hoc SQL outside that adapter.

The adapter MUST:

- verify and record the SQLite engine, repository-format, schema, and storage
  driver versions at startup;
- enable and verify `journal_mode=WAL`, use `synchronous=FULL` unless a later ADR
  proves a stronger or equivalent policy, and fail closed if the requested mode
  cannot be established;
- use bounded busy/lock handling and return a stable conflict/resource error
  rather than waiting without limit;
- serialize repository mutations through the Core writer path while allowing
  bounded readers; WAL is not permission for clients or adapters to perform
  uncoordinated direct writes;
- use parameterized statements, foreign-key enforcement, explicit transactions,
  and compare-and-swap predicates for refs and leases;
- expose the durability levels in `docs/reliability/CONSISTENCY.md`; SQL commit
  success alone is not reported as `local durable` until the configured sync
  boundary is satisfied;
- keep WAL, journal, temporary, and migration files inside the repository
  integrity and secret-scan boundary;
- provide fault-injection hooks for short writes, process termination, lock
  contention, WAL-tail damage, quota exhaustion, permission changes, and schema
  migration interruption.

The repository format is SQLite/schema based, not Rust based. The CAS, manifests,
event envelopes, IDs, redaction profile, and migrations remain governed by their
existing ADRs and protocol documents.

### M1 gate and driver selection

This decision authorizes design and test work, not production code before M1.
`rusqlite` is the v0.x production target; it may not be silently replaced by
`node:sqlite`, `better-sqlite3`, Python `sqlite3`, or a system SQLite library if a
test fails.

M1 MUST establish, on every declared platform/filesystem:

1. WAL and `synchronous=FULL` survive the FI-01 through FI-14 scenarios in
   `M1_DURABLE_PRIMITIVES_GATE.md`.
2. Contract, property, and compatibility tests pass against the adapter port.
3. Recovery is repeatable and does not fabricate terminal success.
4. The bundled SQLite version supports all required SQL features and migration
   fixtures.
5. Startup rejects an incompatible repository or driver with a stable error.

If these conditions cannot be met, implementation stops and a superseding ADR
must select a replacement adapter or change the durability claim. A fallback is
not chosen during an incident because that would make the repository's recovery
semantics depend on an untested driver.

## Consequences

### Positive

- The Core ships as a controlled local process rather than requiring Python or
  Node to be installed on every target host.
- Rust's ownership and type checking reduce memory-safety and resource-lifetime
  hazards in the repository and recovery paths.
- A bundled SQLite engine makes SQL behavior and feature availability more
  reproducible than a system library or an unconstrained runtime release.
- The Python SDK and Node framework integrations remain first-class protocol
  clients; they are not coupled to Rust or SQLite internals.
- The adapter port can later host another driver or a remote metadata service
  while preserving the same command, event, idempotency, and recovery contracts.

### Costs and risks

- Contributors need Rust tooling, and CI/release automation must build and test
  multiple platform artifacts.
- Bundling SQLite increases build work and requires tracking SQLite security and
  feature updates explicitly.
- SQLite WAL and directory-sync behavior still varies by filesystem and power
  failure model; Rust does not make those guarantees universal.
- A protocol process boundary adds startup and IPC overhead compared with an
  embedded Python or Node library. v0.x accepts that cost for isolation and
  language-neutrality.
- Rust Core cannot capture an agent action merely because it is written in Rust;
  capture remains the runtime/adapters' responsibility under ADR-0009.

## Migration and replacement strategy

1. Keep the `MetadataStore`/event-store port and all contract fixtures independent
   of `rusqlite`.
2. Record `storage_driver`, SQLite engine version, schema version, repository
   format, and redaction profile in the project manifest and recovery evidence.
3. A future Node or Python binding must pass the same golden, property, and fault
   suites and must read the existing repository format before it can be offered.
4. Driver replacement is a repository migration with preflight, backup, durable
   generation markers, integrity verification, and restartable recovery. It is
   never an in-place rewrite of immutable events or CAS objects. The concrete
   cross-file selector contract is defined by
   [`ADR-0014`](ADR-0014-repository-generation-migration.md).
5. A future PostgreSQL/remote driver may implement the same port only after an
   ADR defines its consistency, offline behavior, and replication semantics.

## Rejected alternatives

- **Node built-in `node:sqlite` as the authority:** attractive dependency profile,
  but its release-line/API availability and Node installation requirement are not
  a sufficient long-term durability contract for the first repository format.
- **Node with `better-sqlite3` as the authority:** technically workable, but the
  native-addon distribution and ABI surface move maintenance risk into every
  release without improving the protocol boundary.
- **Python Core with `sqlite3`:** a credible alternative and still suitable for
  SDKs/tools, but it gives up the single-binary Core and does not remove SQLite
  version/filesystem variability.
- **Async database abstraction (`sqlx`, callback SQLite APIs):** unnecessary for
  the local v0.x write path and makes transaction, shutdown, and crash ordering
  harder to reason about before scale requires it.
- **System SQLite or a custom storage engine:** produces unbounded platform
  variation or duplicates tested database behavior without evidence of need.

## Follow-up work

- Add the Rust toolchain, target matrix, and minimum supported toolchain to the
  implementation manifest before M1 implementation begins.
- Define the language-neutral metadata/event-store port and its error/status
  mapping before writing the `rusqlite` adapter.
- Select and pin the Rust test runner, assertion/property-testing libraries, and
  the explicit adapter import boundary before adding executable tests described
  in [`tests/README.md`](../../../tests/README.md).
- Add real SQLite WAL/fsync crash tests, including Windows and at least one Unix
  filesystem, to the M1 evidence package.
- Update the roadmap and development guide to identify Rust Core, protocol SDKs,
  and the no-direct-database-access rule.

## References

- [`ADR-0002-local-first.md`](ADR-0002-local-first.md)
- [`ADR-0007-storage.md`](ADR-0007-storage.md)
- [`ADR-0008-event-model.md`](ADR-0008-event-model.md)
- [`ADR-0010-concurrency.md`](ADR-0010-concurrency.md)
- [`ADR-0011-security.md`](ADR-0011-security.md)
- [`M1_DURABLE_PRIMITIVES_GATE.md`](../../development/M1_DURABLE_PRIMITIVES_GATE.md)
- [`STORAGE_ARCHITECTURE.md`](../../architecture/STORAGE_ARCHITECTURE.md)
- [`PONG_PROTOCOL.md`](../../protocol/PONG_PROTOCOL.md)
- [`API_DESIGN.md`](../../protocol/API_DESIGN.md)
- [`M1 Test Harness and local evidence`](../../../tests/README.md)
