# M1 Durable Primitives Gate

**Status: Normative implementation gate; not passed.** M1 has an internal Rust scaffold, but no primitive is released or considered complete until this document's evidence requirements pass. This document is the acceptance contract for the first production-capable Pong primitives. It follows [`docs/roadmap/NEXT_TASK.md`](../roadmap/NEXT_TASK.md) and refines the storage, consistency, recovery, idempotency, security, and versioning decisions in the linked documents below.

## Purpose

M1 proves that Pong can durably record and recover its own control state before workspace, operation, commit, checkpoint, rollback, or replay features are built on top of it. The gate is deliberately narrower than a complete workspace provider or security sandbox.

M1 covers:

1. Content-addressed immutable objects (CAS).
2. Transactional metadata and compare-and-swap ref updates.
3. Append-only events and the write-ahead journal (WAL).
4. Request idempotency and duplicate delivery handling.
5. Crash startup and projection recovery.
6. Redaction before any durable boundary.
7. Schema and repository-format evolution.

M1 does not prove that Pong can observe every host action, reverse an external side effect, or provide distributed consensus. Those remain the boundaries in [`docs/architecture/ARCHITECTURE_REVIEW.md`](../architecture/ARCHITECTURE_REVIEW.md), [`docs/security/TRUST_BOUNDARY.md`](../security/TRUST_BOUNDARY.md), and [`ADR-0011`](../decisions/ADR/ADR-0011-security.md).

## Gate Rule

Production implementation is **blocked** until all items marked **MUST PASS** have executable evidence. A test that is only described, manually run once, or green only with fault injection disabled is not evidence.

The gate may be changed only by an ADR that states the changed invariant, affected compatibility/recovery behavior, migration plan, and replacement tests. A failed test cannot be made non-blocking by relabeling the feature. Experimental PoCs may use test-only code, but no production package may depend on an unproven primitive.

The gate is open only when:

- all contract, property, and fault-injection rows below are green on every supported local filesystem/platform;
- raw repository inspection finds no configured secret in metadata, WAL, object, artifact, temporary, or exported event bytes;
- recovery is repeatable and idempotent from every injected crash point;
- current and previous supported schema fixtures pass compatibility tests;
- failures are classified with a stable error/status, never silently converted to success;
- test evidence records repository format, protocol/schema versions, platform, seed, fault point, and observed result;
- the platform/filesystem and old-reader matrices have a named release-owner decision, date, and retained record; and
- the performance/capacity budget has an accepted ADR or an explicit owner-approved exception with a compensating bound.

Performance measurements and capacity limits must be recorded before M1 exit,
and their acceptance disposition must be retained in an ADR. A proposed or
missing budget does not authorize a release claim or unlimited-scale
assumption.

## Normative Terms and Levels

The terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative. Durability uses the levels in [`docs/reliability/CONSISTENCY.md`](../reliability/CONSISTENCY.md):

- **Local durable**: bytes and the corresponding journal record have reached the configured durability boundary (including required `fsync`/equivalent).
- **Committed**: metadata transaction and all referenced immutable objects are durable and visible through the ref/query boundary.
- **Observed**: a consumer has read through a declared stream sequence.
- **Reconciled**: provider/workspace evidence agrees with the recorded state.

An acknowledgement MUST name the achieved level where the distinction matters.

## M1 Invariants

### Identity and canonical encoding

- **I-IDENT-01 (MUST PASS):** IDs are opaque, typed, and immutable. Hashes and IDs are computed from one canonical serialization; equivalent input cannot produce two digests.
- **I-IDENT-02 (MUST PASS):** Canonical serialization is deterministic across process restarts and supported platforms. It has no locale, map-order, timezone, or floating-point ambiguity.
- **I-IDENT-03 (MUST PASS):** A digest mismatch is an integrity error and never a successful read or publication.

### CAS object store

- **I-CAS-01 (MUST PASS):** The object key is the digest of the exact stored bytes plus the declared object/schema domain. `put` verifies the digest before publication.
- **I-CAS-02 (MUST PASS):** Publication is atomic: readers see either no object or a complete verified object, never a partial object. Staging names are not reachable object names.
- **I-CAS-03 (MUST PASS):** An existing key is immutable. Identical bytes are a successful no-op; different bytes return `INTEGRITY_ERROR`/collision and do not overwrite.
- **I-CAS-04 (MUST PASS):** A committed metadata record can reference only verified durable objects. Unreachable staged/orphan objects are safe to quarantine or garbage-collect after the retention rules allow it.
- **I-CAS-05 (MUST PASS):** Read verifies size and digest (and signature when configured) before returning bytes. Corrupt objects are quarantined and block dependent publication.

### Metadata transactions and refs

- **I-META-01 (MUST PASS):** A metadata transaction is all-or-nothing for its declared rows: no visible partial ref, request outcome, lease, event-index, or projection update.
- **I-META-02 (MUST PASS):** Ref/lease writes use expected-old-value or epoch compare-and-swap. A stale writer cannot replace a newer value; it receives a stable conflict such as `STALE_HEAD` or `CONFLICT`.
- **I-META-03 (MUST PASS):** A committed transaction remains readable after process restart. An uncommitted transaction leaves no success-visible mutation.
- **I-META-04 (MUST PASS):** The local metadata store is authoritative in v0.x. Cache files and human-readable ref mirrors can be rebuilt and never override authoritative rows.
- **I-META-05 (MUST PASS):** Transaction retry with the same request identity is observationally idempotent; retry with a changed command digest is rejected.

### Event stream and WAL

- **I-WAL-01 (MUST PASS):** For a mutating operation, intent is durably journaled before execution. If the outcome is absent at recovery, the operation is `unknown`, never inferred as success.
- **I-WAL-02 (MUST PASS):** An event is append-only and immutable. Per-stream sequence numbers are monotonic and unique; wall-clock timestamps never define ordering.
- **I-WAL-03 (MUST PASS):** Event publication and its state-delta/index update are atomic at the metadata visibility boundary. Blob bytes may precede publication but cannot be referenced before verification.
- **I-WAL-04 (MUST PASS):** Duplicate append/delivery converges by `event_id` and `(stream_id, sequence)`. Conflicting duplicates fail closed and are retained as integrity evidence.
- **I-WAL-05 (MUST PASS):** A torn/truncated WAL tail is detected. Complete records before the tail remain readable; the damaged tail is quarantined or repaired only by an explicit recovery action.
- **I-WAL-06 (MUST PASS):** Causation, correlation, request, actor, schema version, redaction profile, and integrity metadata survive restart and projection rebuild.

### Idempotency and unknown effects

- **I-IDEMP-01 (MUST PASS):** The idempotency key is `(project_id, actor_id, request_id)` for the retention window. The canonical command digest is stored with the result.
- **I-IDEMP-02 (MUST PASS):** Repeating the same key and digest returns the original result/status without a second metadata mutation or unsafe second tool execution.
- **I-IDEMP-03 (MUST PASS):** Reusing a key with a different digest returns `IDEMPOTENCY_KEY_REUSE` and does not mutate state.
- **I-IDEMP-04 (MUST PASS):** A transport/process failure around an external call yields `SIDE_EFFECT_UNKNOWN` unless provider evidence proves a terminal outcome. Unknown is never silently retried.
- **I-IDEMP-05 (MUST PASS):** Duplicate event consumers and recovery projections are idempotent; applying the same input zero, one, or many times converges to the same projection.

### Recovery

- **I-REC-01 (MUST PASS):** Startup verifies repository format, journal integrity, reachable object integrity, refs, and migration markers before reporting healthy.
- **I-REC-02 (MUST PASS):** Recovery replays complete journal records idempotently, marks orphan intents `unknown`, and emits reconciliation evidence where provider/workspace scans are available.
- **I-REC-03 (MUST PASS):** Recovery never fabricates terminal success, rewrites immutable history, or advances a ref without expected-old-value checks.
- **I-REC-04 (MUST PASS):** A repeated or interrupted recovery run converges to the same state. Recovery exposes `recovery_required`, `reconciling`, `degraded`, or `quarantined` status instead of hiding uncertainty.
- **I-REC-05 (MUST PASS):** Restore/recovery primitives preserve original events and create new history. External effects and excluded resources remain explicitly excluded.

### Redaction and secret handling

- **I-RED-01 (MUST PASS):** Redaction executes before journal append, metadata binding, CAS/artifact publication, logs, metrics labels, test fixtures, and exports. There is no post-persistence cleanup as a security control.
- **I-RED-02 (MUST PASS):** Configured secrets, credentials, cookies, authorization headers, private keys, secret-bearing arguments, and allowlist-excluded environment values never appear in ordinary persisted bytes.
- **I-RED-03 (MUST PASS):** Redaction is deterministic for equivalent input and emits a stable marker/profile version without allowing recovery of the original value from the marker, digest, length, or surrounding context.
- **I-RED-04 (MUST PASS):** Secret-bearing operations require the separate capability/policy described in [`docs/security/SECURITY_MODEL.md`](../security/SECURITY_MODEL.md); access and denial are audited without persisting the secret.
- **I-RED-05 (MUST PASS):** A redaction failure fails closed. It MUST NOT downgrade to raw persistence or claim a clean capture.

### Schema and repository evolution

- **I-SCHEMA-01 (MUST PASS):** Every event, operation, object, projection, and project manifest carries an explicit schema/object/repository version as applicable.
- **I-SCHEMA-02 (MUST PASS):** Historical events and immutable objects are never rewritten in place. Migrations create a new view/generation, with preflight, backup, integrity verification, and a recovery marker.
- **I-SCHEMA-03 (MUST PASS):** Current and previous supported schema fixtures round-trip through readers and projections. Unsupported major versions fail before mutation with `UNSUPPORTED`.
- **I-SCHEMA-04 (MUST PASS):** Unknown optional fields and open enum values are preserved as opaque data where policy permits; required-field removal and semantic changes require a major version and ADR.
- **I-SCHEMA-05 (MUST PASS):** A crashed migration is restartable and leaves either the old generation or a fully verified new generation visible, never a mixed generation.
- **I-SCHEMA-06 (MUST PASS):** Redaction profile/version is part of the compatibility surface. A reader MUST NOT infer that an older object is safe under a newer redaction policy without an explicit migration/attestation.

## State Machines

### Mutating operation and journal

```text
validated -> authorized -> reserved -> intent_durable -> executing
  -> outcome_durable -> published

validated/authorized failure -> rejected (no execution mutation)
executing -> failed | cancelled | unknown
outcome_durable -> unreconciled (if publication/indexing is unavailable)
unreconciled -> published | unknown (during recovery/reconciliation)
```

`unknown` is a terminal recording status until provider/workspace evidence or an explicit operator decision creates a new reconciliation event. It is not an invitation to retry blindly.

### CAS object

```text
absent -> staging -> verified -> durable -> reachable
durable -> orphaned -> quarantine | garbage_candidate -> deleted
reachable -> quarantine (integrity failure; dependent publication blocked)
```

Only `durable` verified objects may be referenced. `deleted` is allowed only after mark-and-sweep retention checks.

### Metadata transaction

```text
open -> prepared -> committed
open/prepared -> aborted
crash at any point -> recovery decides committed or aborted from the journal
```

Readers observe rows only after `committed`. Recovery MUST NOT guess when the journal cannot distinguish the outcome; it reports `recovery_required`.

### Repository recovery

```text
healthy -> recovery_required -> reconciling -> healthy
                         \-> degraded
                         \-> quarantined (integrity/security failure)
```

Entering `degraded` blocks new irreversible operations unless policy explicitly allows them. `quarantined` blocks publication until operator action.

## Contract Test Matrix

Contract tests use fixed, redacted fixtures and validate public ports, envelopes, schemas, error codes, and capability negotiation. Every row is **MUST PASS**.

| ID | Area | Required contract | Evidence |
| --- | --- | --- | --- |
| CT-01 | Canonical identity | Same canonical input yields the same typed ID/digest; malformed IDs and non-canonical encodings are rejected. | Golden vectors across supported platforms. |
| CT-02 | CAS | `put/get/exists` verify digest, size, object type, schema version, and immutable collision behavior. | Object-store port fixture and raw-byte verification. |
| CT-03 | Metadata | Transaction commit/abort, expected-old-value ref update, lease epoch, and typed conflict errors match the protocol. | Metadata adapter fixture and SQL/driver-independent assertions. |
| CT-04 | Event/WAL | Envelope fields, per-stream sequence, causation/correlation, integrity metadata, append result, and cursor semantics are stable. | Versioned event fixtures and export/import round trip. |
| CT-05 | Idempotency | Same request ID plus digest returns the original result; changed digest returns `IDEMPOTENCY_KEY_REUSE`. | Golden command/result fixtures, including failure results. |
| CT-06 | Recovery | Startup status, orphan intent classification, corruption/quarantine, and recovery-required errors are stable and machine-readable. | Recovery fixture corpus with expected status and event set. |
| CT-07 | Redaction | Secrets are replaced before persistence/export; markers and profile versions are stable; denial is fail-closed. | Raw `.pong` byte scan plus safe-output fixture comparison. |
| CT-08 | Schema evolution | Current/previous readers, additive fields, unknown enums/extensions, unsupported major versions, and migration markers behave as specified. | Compatibility fixture matrix and migration manifest. |
| CT-09 | Security/audit | Capability decisions, actor/request IDs, redaction metadata, and irreversible-operation approvals are present without raw credentials. | Authorized/denied command fixtures and audit assertions. |

## Property Test Matrix

Properties use deterministic seeds and a generated corpus (default minimum: 10,000 cases per property in CI; seed and shrink trace are retained on failure). Every row is **MUST PASS**.

| ID | Property | Required assertion |
| --- | --- | --- |
| PT-01 | Canonical hashing | Equivalent maps, sets, and timestamps normalize to one digest; changing any covered byte changes the digest. |
| PT-02 | CAS convergence | Repeated/concurrent puts of identical bytes converge to one object; no successful read returns bytes with a different digest. |
| PT-03 | Metadata atomicity | For arbitrary transaction operation sequences, readers observe either the pre-state or the full post-state, never a partial subset. |
| PT-04 | Ref/lease CAS | With arbitrary writer interleavings, at most one writer wins an expected-head/epoch update; stale writers cannot advance the ref. |
| PT-05 | Event ordering | Appends produce contiguous monotonic per-stream sequences; cross-stream consumers rely only on causal links/cursors, not timestamps. |
| PT-06 | Duplicate event delivery | Applying any permutation and repetition of a valid event batch converges to one projection and one integrity outcome. |
| PT-07 | Idempotent commands | Retrying any command with the same key/digest produces one observable result and no additional state transition. |
| PT-08 | Key reuse rejection | Any changed command bytes, actor, or project scope cannot reuse an existing idempotency record. |
| PT-09 | Recovery convergence | Running recovery zero, one, or many times, including interruption between records, converges to the same state and event set. |
| PT-10 | Unknown preservation | No generated crash schedule turns an intent without durable outcome into `succeeded`. |
| PT-11 | Redaction closure | For generated secret placements in fields, nested payloads, argv, environment, logs, and artifacts, the raw secret is absent from every persisted/exported byte. |
| PT-12 | Redaction determinism | Equivalent input and profile produce the same marker/profile metadata; redaction never expands a secret into another persisted representation. |
| PT-13 | Schema projection | Migrating supported fixtures forward/backward within the compatibility window preserves required semantics and opaque extensions. |
| PT-14 | Migration atomicity | Generated migration interruption exposes only the old or fully verified new generation; repeated migration is idempotent. |

## Fault-Injection Matrix

The harness MUST be able to terminate the process, fail a filesystem operation, truncate a journal tail, exhaust quota, revoke permissions, and corrupt an object at each named point. Each case runs repeatedly with a deterministic seed and is inspected after a cold restart.

| ID | Injection point | Expected result | Gate |
| --- | --- | --- | --- |
| FI-01 | Before intent fsync | No execution is allowed; request is retryable or rejected with no success event. | MUST PASS |
| FI-02 | After intent fsync, before tool execution | Intent survives; recovery marks the operation `unknown` or an explicit non-executed cancellation, never `succeeded`. | MUST PASS |
| FI-03 | After tool execution, before outcome append | Operation is `unknown` until provider/workspace evidence reconciles it; blind retry is blocked. | MUST PASS |
| FI-04 | During outcome append/tail truncation | Complete prior records survive; damaged tail is detected/quarantined; operation is `unknown` or `unreconciled`. | MUST PASS |
| FI-05 | After outcome durable, before event index/ref transaction | Outbox/recovery republishes idempotently; no duplicate event or ref advance occurs. | MUST PASS |
| FI-06 | During metadata transaction commit | Readers see all pre-state or all post-state; recovery chooses from durable journal evidence. | MUST PASS |
| FI-07 | During CAS write (short write/ENOSPC) | Staging object is unreachable/quarantined; no partial object is readable or referenced. | MUST PASS |
| FI-08 | After CAS rename, before directory fsync | Restart verifies reachability/integrity; publication waits or fails closed if durability is unproven. | MUST PASS |
| FI-09 | After ref CAS, before response | Retry returns the committed original result; stale or duplicate ref changes are rejected. | MUST PASS |
| FI-10 | During projection rebuild | Rebuild can restart; projection converges from immutable events; source events remain unchanged. | MUST PASS |
| FI-11 | During redaction or export | Operation fails closed; no raw secret reaches journal, WAL, temp file, log, artifact, or export. | MUST PASS |
| FI-12 | During schema migration | Old/new generation marker is recoverable; mixed schema visibility is impossible; rerun is safe. | MUST PASS |
| FI-13 | Permission revocation or read corruption | Integrity/security status is `quarantined` or `recovery_required`; no unsafe publication. | MUST PASS |
| FI-14 | Disk full during journal/object/metadata write | Existing committed history remains readable; new mutation returns `RESOURCE_EXHAUSTED` or recovery status without false success. | MUST PASS |

## Local FI-12 Evidence

The Windows development harness now exercises the v0.1-to-v0.2 cross-file
repository migration in `tests/repository_migration.rs` and
`tests/process_kill_migration.rs`. It proves SQLite Online Backup, immutable
legacy preservation, target manifest/profile verification, atomic selector
visibility, the target-allocation/backup/checkpoint/verify/selector failpoints,
selector-journal completion after cold restart, and child-process termination
on both sides of selector replacement. Every covered interruption reopens
old-only or new-only; no test adopts a directory merely because it exists.

A concurrent PT-14 run reproduced Windows raw access-denied classification bugs
at directory sync and journal publication; protected repository boundaries now
map both paths to `PERMISSION_DENIED`, and three concurrent reruns plus the
stable/MSRV default-case property suites pass. Retained Windows stable reruns
now provide completed 10,000-case evidence for PT-09 and PT-10; the aggregate
storage-heavy record remains a historical no-result artifact, and PT-14's
normative 10,000-case continuation is still incomplete.

This is local executable evidence for I-SCHEMA-02, I-SCHEMA-05, CT-08, and
FI-12. The current evidence package also contains a retained 10,000-case
property subset, Windows ACL FI-13 evidence, Linux `tmpfs` ENOSPC FI-14 evidence,
and a measurement-only performance artifact. Those observations do not close
the normative gate: a declared supported-platform matrix, separately released
old-reader matrix, complete fault disposition, and accepted
performance/capacity budgets remain required.

## Evidence and Review Artifacts

The M1 test harness and review package MUST contain:

- the primitive port contracts and versioned golden fixtures;
- deterministic fake clock, filesystem, fsync, process-kill, quota, and corruption controls (synthetic CAS permission/quota fixtures are documented in [`CAS_FAULT_FIXTURES.md`](CAS_FAULT_FIXTURES.md) and do not replace real platform evidence);
- raw-byte secret scans over the full `.pong` tree, including SQLite WAL and staging files;
- crash schedule results for every `FI-*` point, with restart logs and expected/actual state;
- a machine-readable FI-01 through FI-14 disposition index that distinguishes
  synthetic coverage, real-host evidence, and external/unimplemented gaps;
- property-test seeds, shrink traces for failures, and rerun commands;
- migration preflight, backup, generation marker, verification, and rollback evidence;
- a report of journal latency, recovery duration, object size/throughput, quota behavior, and test platform;
- an ADR for every invariant that is weakened, deferred, or made provider-specific.

## M1 Exit Checklist

M1 is **PASS** only when every line below is true:

- [ ] All `I-*`, `CT-*`, `PT-*`, and `FI-*` items marked MUST PASS are green.
- [ ] No committed or staged production implementation depends on a red test or an untested fallback.
- [ ] Repeated recovery is convergent and leaves unknown effects explicit.
- [ ] CAS, metadata, WAL, and projections survive supported crash and storage failures without fabricated success.
- [ ] The secret scan finds zero configured-secret occurrences in all persisted and exported bytes.
- [ ] Current and previous schema fixtures pass, and migration interruption has a verified recovery path.
- [ ] Error/status codes, request IDs, sequence/cursor behavior, and redaction metadata match the protocol documents.
- [ ] Any changed architecture assumption has an accepted ADR and updates to `NEXT_TASK.md`, the roadmap, and relevant reliability/security docs.

Until this checklist is complete, the repository remains in test-design/PoC mode and the next production task is not authorized.

## References

- [`docs/architecture/STORAGE_ARCHITECTURE.md`](../architecture/STORAGE_ARCHITECTURE.md)
- [`docs/architecture/EVENT_MODEL.md`](../architecture/EVENT_MODEL.md)
- [`docs/reliability/CONSISTENCY.md`](../reliability/CONSISTENCY.md)
- [`docs/reliability/RECOVERY.md`](../reliability/RECOVERY.md)
- [`docs/reliability/IDEMPOTENCY.md`](../reliability/IDEMPOTENCY.md)
- [`docs/security/SECURITY_MODEL.md`](../security/SECURITY_MODEL.md)
- [`docs/security/TRUST_BOUNDARY.md`](../security/TRUST_BOUNDARY.md)
- [`docs/protocol/PONG_PROTOCOL.md`](../protocol/PONG_PROTOCOL.md)
- [`docs/protocol/VERSIONING_PROTOCOL.md`](../protocol/VERSIONING_PROTOCOL.md)
- [`docs/development/TEST_STRATEGY.md`](TEST_STRATEGY.md)
- [`docs/governance/PROJECT_CONSTITUTION.md`](../governance/PROJECT_CONSTITUTION.md)
