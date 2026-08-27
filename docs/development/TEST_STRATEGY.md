# Test Strategy

## Test layers

1. **Contract tests** validate protocol envelopes, schemas, error codes, capability negotiation, and compatibility fixtures.
2. **Domain tests** cover object invariants, DAG ancestry, snapshot/commit/checkpoint distinctions, and permission decisions.
3. **Storage tests** exercise CAS integrity, transactions, journal replay, corruption detection, and garbage collection.
4. **Runtime tests** use fake tools and crash injection to verify interception, redaction, backpressure, and unknown outcomes.
5. **Integration tests** cover workspace providers, Git projections, and framework adapters.
6. **End-to-end tests** run agent workflows: attach, edit, checkpoint, commit, branch, merge, rollback, replay, and recovery.

## Required properties

- Replaying the same request ID is observationally idempotent.
- Event append is monotonic and duplicate delivery converges.
- A commit references only verified immutable objects.
- A stale ref/lease cannot overwrite a newer writer.
- Redaction prevents configured secrets from appearing in persisted payloads.
- Recovery never fabricates success for an unknown side effect.
- Rollback/replay create new history and preserve originals.

## Failure and security testing

Inject process kills, power-loss-like truncation, disk-full, clock skew, network partitions, adapter omission, malformed payloads, permission revocation, path traversal, oversized artifacts, and malicious serialized state. Verify fail-closed behavior for integrity and irreversible-operation approval.

## Compatibility and performance

Run golden fixtures across the current and previous protocol/schema versions. Measure journal throughput, fsync latency, snapshot cost, recovery time, and large-workspace reconciliation. Performance tests must include backpressure and bounded-resource behavior, not only the happy path.

## Test artifacts

Fixtures contain no secrets and identify their schema/protocol version. Each bug fix adds a regression test and, when relevant, a failure-injection scenario or compatibility fixture. Flaky tests are quarantined with an owner and expiry, never silently ignored.

## Current M1 evidence

The internal Rust crate exercises canonical encoding, CAS publication/quarantine and secret rejection, SQLite WAL/FULL configuration, ref compare-and-swap, idempotency, immutable event metadata, redaction/profile compatibility, repository startup markers, orphan-intent recovery, process-kill and torn-tail restart boundaries, synthetic quota/permission fixtures, a full `.pong` raw-byte scan, and cross-file v0.1-to-v0.2 repository-generation migration. Migration tests use SQLite Online Backup, an atomic root selector, generation manifests, in-database generation/migration identity attestation, and child-process cold restarts; a valid SQLite file bound to the wrong generation is rejected as an integrity error.

The current evidence package also contains a retained fixed-seed 10,000-case
property subset (Windows PT-01/02/03/04/05/06/07/08/11/12), completed Windows
stable PT-09 and PT-10 normative records, and a completed Windows stable PT-14
normative record (10,008 cases across nine migration failpoints), real Windows
ACL FI-13 evidence, disposable Linux `tmpfs` ENOSPC FI-14 evidence for
CAS/metadata/journal writes, and the measurement record in
`artifacts/m1-performance.json`. These runs are valuable executable evidence
but remain bounded to their observed environments; PT-14 normative evidence
is Windows stable NTFS only and is not a cross-platform acceptance. The structured
decision and remaining gaps are maintained in
[`M1_EVIDENCE.md`](M1_EVIDENCE.md); the M1 gate remains closed until the
supported-platform/old-binary matrices, accepted budgets, and complete fault
disposition are accepted against the complete `.pong` tree. The historical
Windows PT-14 selector-sync/journal-publication classification issues have
reproduced root causes and fixes, with concurrent rerun evidence; broader
filesystem coverage remains open.

## Current M2 evidence

The internal Rust workspace slice is tested separately in
`tests/workspace_snapshot.rs`. It covers lease exclusivity and epoch takeover,
stale workspace revisions, deterministic repeated snapshots, local
materialization into a new destination, manager-controlled head advancement,
Unix symlink rejection, and sensitive environment allowlists. The focused M2
acceptance boundary and missing tests are listed in
[`M2_WORKSPACE_SNAPSHOT_GATE.md`](M2_WORKSPACE_SNAPSHOT_GATE.md). M2 tests do
not waive any M1 requirement and are not a public API compatibility suite; the
slice is frozen until the M1 release gate is accepted.
