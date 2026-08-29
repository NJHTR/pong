# M1 Projection Acceptance Package

**Prepared:** 2026-08-28  
**Audited commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**ADR:** [`ADR-0016-projection-contract.md`](../decisions/ADR/ADR-0016-projection-contract.md)  
**ADR status:** `Proposed`  
**Acceptance status:** `OWNER_ACTION_REQUIRED`

This package maps the proposed ADR-0016 contract to the implementation and
retained evidence. It is not an acceptance of the ADR.

## Acceptance Matrix

| Requirement | Implementation | Tests | Evidence | Acceptance decision |
| --- | --- | --- | --- | --- |
| Immutable versioned event envelope and opaque additive fields | Additive envelope schema with canonical payload/integrity metadata | `tests/event_projection.rs`; malformed/unknown envelope cases in `tests/pt13_fi10.rs` | Current-host targeted summary; GitHub-hosted Run `33145714975` focused logs | Owner review required |
| Generation/projection/cursor identity | Generation-bound projection state, schema/migration/redaction identity, project-local cursor | `tests/event_projection.rs`; generation isolation and stale cursor tests in `tests/pt13_fi10.rs` | `artifacts/m1-pt13-fi10-summary.json` / `.log`; Linux-native and hosted focused execution | Owner review required |
| Deterministic idempotent handlers and duplicate/conflict behavior | Handler registry, applied-event ledger, digest checks, degraded unknown-event policy | Duplicate event identity, conflicting payload, idempotency, and unknown-event tests | Current-host and Run `33145714975` focused evidence | Owner review required |
| Atomic apply/publication of state, cursor, ledger, and digest | Metadata transaction and generation-bound publication boundary | Projection apply/rebuild and corruption tests | `tests/event_projection.rs`; `tests/pt13_fi10.rs` | Owner review required |
| Rebuild, high-water verification, and cold-reopen recovery | Staging rebuild, replay verification, retry convergence | FI-10 A-J schedule, repeated crash/reopen, clean replay tests | `artifacts/m1-release-evidence/logs/m1-pt13-fi10-summary.log`; Linux-native and hosted focused rows | Owner review required |
| Generation migration ordering and old-or-fully-verified-new visibility | Target generation identity checks before selector publication | A-to-B migration, interruption, foreign-generation rejection | `tests/pt13_fi10.rs`; `tests/repository_migration.rs`; `tests/process_kill_migration.rs` | Owner review required |
| Source-event immutability | Source events remain authoritative and unchanged during projection/rebuild | Migration semantic preservation and source-event immutability assertions | PT-13/FI-10 retained summary and focused logs | Owner review required |

## Evidence Boundary

The retained implementation evidence is strong for the exercised current
Windows host, Linux-native VM records, and GitHub-hosted Linux/macOS focused
jobs. The hosted macOS filesystem is `unknown`; it is not APFS or a physical
Mac claim. These are executable evidence rows, not a substitute for the ADR
decision.

## Required Owner Decision

The Core/Release Owner must review this matrix against ADR-0016 and record:

- `Accepted` or `Rejected`;
- exact M1 platform/filesystem scope;
- accepted evidence and any excluded/deferred behavior;
- decision date and owner identity; and
- residual risks and follow-up conditions.

ADR-0016 must remain `Proposed` until that decision is recorded. There is no
waiver while projections remain in M1; removing projections would be a formal
scope/roadmap change.

```text
TECHNICAL EVIDENCE = PRESENT for the recorded implementation scope
ADR-0016 = Proposed
ACCEPTANCE = OWNER_ACTION_REQUIRED
```
