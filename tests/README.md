# M1 Test Harness

**Status: local Rust evidence is executable; the M1 gate is still pending.**

This directory records the executable-test contract for M1 Durable Primitives. It
does not contain a fake adapter that can make the gate green. ADR-0013 selects a
Rust library/runtime with `cargo test` and an explicit primitive-port adapter;
production evidence is still absent until those tests exercise the real ports.

## Entry criteria for test code

Before adding the full harness, the accepted implementation ADR requires:

- Rust version and supported target platforms;
- `cargo test`, deterministic generators, and the property-testing approach;
- crate/module layout and the public adapter import boundary;
- filesystem/platform support and crash-injection capabilities;
- fixture serialization and compatibility policy.

The ADR must reference this directory and state how a test failure maps to an
`I-*`, `CT-*`, `PT-*`, or `FI-*` row in
[`docs/development/M1_DURABLE_PRIMITIVES_GATE.md`](../docs/development/M1_DURABLE_PRIMITIVES_GATE.md).

## Planned layers

The Rust harness is split into these layers:

1. **Contract tests** consume a versioned primitive-port adapter and assert
   envelopes, schemas, IDs, cursors, and stable error codes.
2. **Property tests** generate deterministic command/event/object sequences. Each
   failure records the seed and shrink trace.
3. **Fault tests** run one named failpoint per cold restart. They must inspect the
   repository on disk, including journal tails, WAL files, staging objects, and
   temporary files.
4. **Compatibility tests** read current and previous fixtures, including unknown
   additive fields and open enum values.
5. **Security tests** scan every persisted/exported byte for registered secrets;
   redaction tests must fail closed when redaction itself fails.

Repository-generation migration tests additionally inspect the on-disk
`.pong` tree after each real migration attempt. They use SQLite Online Backup,
the generation manifest, the root selector, and a process-kill child entrypoint
to prove that cold restart exposes the legacy generation or the fully verified
active generation, never a mixed directory. Fixture JSON shape checks remain
supplementary and are not counted as FI-12 evidence by themselves.

Tests must receive an adapter through an explicit port. They must not import an
internal database, call a production singleton, disable fault injection, or
replace an unimplemented behavior with a passing TODO.

## Fixture contract

The language-neutral fixture envelope is specified in
[`fixtures/fixture.schema.json`](fixtures/fixture.schema.json). Fixtures are
redacted, deterministic, and safe to commit. Every fixture records:

- its fixture, protocol, schema, and repository-format versions;
- the M1 row(s) it exercises;
- a canonical input payload and machine-readable expected result;
- fault-point and restart metadata when it is a fault fixture;
- evidence metadata (platform, seed, and observed status) when produced by a run.

The JSON schema cannot prove that a secret is absent. A harness-level raw-byte
scan remains mandatory and must cover the complete `.pong` tree plus exports.

CAS permission and quota fixtures are synthetic and instance-scoped. They are
required for deterministic Windows coverage, but they do not replace a real
platform run with ACL revocation or an exhausted filesystem. See
[`docs/development/CAS_FAULT_FIXTURES.md`](../docs/development/CAS_FAULT_FIXTURES.md)
for the evidence boundary and required unsupported-platform recording.

## Naming and evidence

Use stable names such as `CT-01-canonical-id.json`, `PT-09-recovery.json`, and
`FI-04-wal-tail.json`. A generated failure report should include the fixture ID,
M1 row, implementation/repository version, platform, seed, failpoint, expected
status, observed status, and a path to the cold-restart inspection output.

The current tests are local evidence for the rows named in their fixtures and
test names. The package now includes a retained fixed-seed 10,000-case property
subset (PT-01/02/03/04/05/06/07/08/11/12), completed Windows stable normative
records for PT-09, PT-10, and PT-14, opt-in Windows ACL FI-13 evidence,
disposable Linux `tmpfs` FI-14 ENOSPC evidence for CAS/metadata/journal writes,
and a measurement-only performance artifact. PT-14's normative record covers
Windows stable NTFS only; the bounded/default probes remain supplemental.
`artifacts/m1-fault-matrix.json` is the retained disposition index
for FI-01 through FI-14; it distinguishes synthetic coverage from real-host
and unimplemented gaps. They still do not satisfy the full M1 gate: the
declared supported platform/old-binary matrices, complete fault disposition,
and accepted performance/capacity budget remain required.

The focused FI-01 and FI-08 tests additionally drop their file-backed stores
and reopen them cold after the injected boundary. They prove the local
pre-state/post-publication invariant and retry behavior; they remain synthetic
failpoint evidence and are not substitutes for power-loss or native filesystem
durability schedules.

The migration security regression
`repository_migration::foreign_valid_generation_database_cannot_replace_the_active_identity`
copies a separately valid generation-format SQLite file from another repository
into the active path. Startup must reject its foreign durable
generation/migration identity with `INTEGRITY_ERROR`; format-valid bytes alone
cannot bypass selector binding.

`artifact_consistency.rs` recursively verifies retained M1 artifact JSON records:
every declared raw log exists under the repository, its SHA-256 matches, and
every `retained_artifacts` path in the FI matrix exists. This is evidence-package
integrity only; it does not accept an artifact or change the release gate.

### Property corpus

`property_corpus.rs` covers the implemented canonical/CAS/metadata/ref/event,
idempotency, and redaction subsets (PT-01/02/03/04/05/06/07/08/11/12).
`property_recovery_migration.rs` covers generated recovery, unknown-outcome,
and generation-selector schedules (PT-09/10/14). `event_projection.rs` covers
the implemented envelope, project cursor, idempotent apply, degraded unknown
events, and rebuild convergence. Both suites use fixed ChaCha seeds, proptest
shrinking, and persisted regression files under `proptest-regressions/`; CI or
`PONG_PROPTEST_CASES=10000` runs the normative case count. PT-13 migration
semantics and FI-10 crash schedules remain unclaimed until dedicated fixtures
and cold-reopen evidence are added.

### Measurement probe

The ignored `performance_measurements.rs` test runs a fixed repository-open,
metadata, event, recovery, CAS, eight-sample migration, and local snapshot
scale workload and emits an `m1-perf-0.3` JSON record with raw samples,
p50/p95/p99/max summaries, and platform/seed metadata:

```text
PONG_PERF_OUTPUT=artifacts/m1-performance.json cargo test --locked --test performance_measurements -- --ignored --nocapture
```

The output is measurement evidence only; it does not invent a release budget or
replace the supported-platform and host-resource matrices.
