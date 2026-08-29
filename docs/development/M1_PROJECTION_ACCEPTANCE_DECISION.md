# Pong M1 Projection Acceptance Decision

**Status:** `ACCEPTED`  
**Owner:** `NJHTR`  
**Date:** `2026-08-29`  
**Formal ADR-0016 acceptance:** `ACCEPTED`  
**ADR:** `ADR-0016-projection-contract.md` (remains `Proposed`)

```text
Implementation = PASS
Tests = PASS
Evidence = PASS
Acceptance = ACCEPTED BY OWNER
```

This is the Owner acceptance record for the M1 Projection Contract. It does
not modify the source ADR file or create a release candidate.

## Requirement Review

| Requirement | Implementation | Tests | Evidence | Risk | Owner Decision |
| --- | --- | --- | --- | --- | --- |
| Versioned immutable event envelope with opaque additive fields | Event envelope and source-event persistence | `tests/event_projection.rs`, `tests/pt13_fi10.rs` | Current-host tests and focused Run `33145714975` records | No additional risk accepted beyond stated contract | `ACCEPTED` |
| Generation-bound projection, cursor, ledger, schema, migration, and redaction identity | Projection identity and generation checks | Projection identity and generation-isolation cases | PT-13/FI-10 retained artifacts | Future schema changes require new review | `ACCEPTED` |
| Deterministic, idempotent handlers and explicit unknown-event behavior | Rebuild and unknown-event paths | Idempotency, rebuild, and unknown-event tests | Current-host and focused hosted evidence | Unsupported future schema policy remains bounded by ADR | `ACCEPTED` |
| Atomic event, state, cursor, and digest publication | Transactional projection apply/publication | Duplicate/conflict and digest tests | `tests/event_projection.rs` and retained PT-13 evidence | Contract remains limited to tested scope | `ACCEPTED` |
| Rebuild staging, high-water verification, cold reopen, and old-or-verified-new visibility | Rebuild/checkpoint/recovery implementation | FI-10 A-J failpoint and repeated-crash cases | `tests/pt13_fi10.rs`, Run `33145714975` | Native scope is the retained evidence scope | `ACCEPTED` |
| Source-event immutability during migration/rebuild | Source rows remain immutable | Migration and source-integrity cases | PT-13/FI-10 retained evidence | No additional behavior claimed | `ACCEPTED` |

## Technical Evidence

The implementation and executable evidence are present for the recorded
current-host and focused hosted scope. PT-13 and FI-10 are accepted as the
technical basis for the M1 projection contract.

## Owner Decision

The Release Owner explicitly accepted projections in M1 and states:

```text
ADR-0016 = ACCEPTED
```

The acceptance is recorded in the Owner sign-off with scope, date, owner, and
textual declaration. The source ADR file remains `Proposed` because this
execution does not mutate ADR history. Waiver is not available while the
projection contract remains in M1.
