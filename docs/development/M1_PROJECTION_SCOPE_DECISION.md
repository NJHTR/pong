# M1 Projection Scope Decision

**Prepared:** 2026-08-28  
**Audited commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**ADR:** [`ADR-0016-projection-contract.md`](../decisions/ADR/ADR-0016-projection-contract.md)  
**Current status:** `OWNER_ACTION_REQUIRED`

**Owner Decision Draft:** keep projections in M1; `OWNER ACCEPTANCE REQUIRED`  
**Formal ADR-0016 acceptance:** `PENDING`

> **Final acceptance overlay (2026-08-29):** Owner `NJHTR` accepted
> `ADR-0016` as the M1 Projection Contract. The source ADR remains `Proposed`
> in accordance with the instruction not to mutate ADR history. The pending
> labels below are retained proposal history.

This is an acceptance-readiness record. It does not modify ADR-0016 and does
not accept the projection contract.

## Contract in ADR-0016

ADR-0016 defines an internal, generation-bound projection slice with:

- immutable versioned event envelopes and opaque additive fields;
- stable projection and cursor identity bound to generation, schema, migration,
  and redaction identity;
- deterministic, idempotent handlers and explicit unknown-event behavior;
- atomic event apply, state, cursor, and digest publication;
- rebuild staging, high-water verification, cold-reopen recovery, and old-or-
  fully-verified-new generation visibility; and
- source-event immutability throughout migration and rebuild.

Required acceptance evidence includes versioned/golden fixtures, unknown and
unsupported schema vectors, duplicate/conflicting-event tests, cursor ordering,
opaque-field preservation, FI-10 crash boundaries, cold reopen, and source
immutability.

## Technical Evidence

The implementation evidence is present for the recorded current-host scope:

| Evidence | Result | Scope |
| --- | --- | --- |
| `tests/event_projection.rs` | Passes | Current repository host |
| `tests/pt13_fi10.rs` | Passes targeted migration, generation-isolation, malformed/corrupt, A-J failpoint, repeated-crash, and golden-state cases | Current repository host |
| `artifacts/m1-pt13-fi10-summary.json` / `.log` | Three targeted reruns retained | Current Windows host |
| Run `33145714975` focused `pt13-fi10` jobs | Exit code `0` on Linux and macOS GitHub-hosted jobs | Hosted runner evidence; macOS filesystem remains `unknown` |
| Linux VM property/projection execution | Current Linux-native evidence retained | Rust 1.85 VM; not an MSRV acceptance |

These results cover event envelope, project-local cursor, applied-event ledger,
state digest, rebuild, idempotency, generation binding, unknown-event handling,
and the A-J FI-10 schedule for the exercised scope. They are implementation
evidence, not an architectural acceptance decision.

## Acceptance Decision Required

The Core/Release Owner must review the retained fixtures and decide whether
the ADR-0016 contract is the M1 projection contract. The decision must be
recorded with a date, exact scope, accepted evidence, and any rejected or
deferred behavior. ADR-0016 must remain `Proposed` until then.

**WAIVER POSSIBLE = NO** while projections remain in M1 scope. Deferring the
projection slice is possible only through a formal scope/roadmap decision that
removes it from M1; that is a scope change, not a waiver of the contract.

## Current Disposition

```text
TECHNICAL EVIDENCE = PRESENT for current-host and Run 33145714975 focused scope
OWNER DECISION REQUIRED = Accept or reject Proposed ADR-0016
WAIVER POSSIBLE = NO while projections remain in M1
M1 PROJECTION ACCEPTANCE = PENDING
```

## Owner Decision Draft Overlay

The supplied Owner Decision Draft keeps the projection contract in M1 and
approves it for final-candidate preparation. The implementation and PT-13/FI-10
evidence are technically present for the recorded scope. ADR-0016 remains
`Proposed` and requires an explicit human `ACCEPTED` or `REJECTED` decision;
this overlay does not change the ADR or execute acceptance.
