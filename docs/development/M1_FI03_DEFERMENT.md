# Pong M1 FI-03 Deferment

**Status:** `ACCEPTED SCOPE DEFERMENT`  
**Owner:** `NJHTR`  
**Date:** `2026-08-29`  
**Formal scope acceptance:** `ACCEPTED`  
**Disposition:** `DEFERRED TO M2+`

## Boundary

M1 Durable Core does not expose external provider execution semantics. FI-03's
external-side-effect scenario requires a real effect,
termination after the effect and before outcome append, then reconciliation on
cold reopen. That boundary is not present in the current Core.

## Evidence Status

Existing PT-10 and related schedules are synthetic local intent/unknown
preservation evidence. They are useful groundwork but do not prove a real
external provider effect or reconciliation. FI-03 is not `PASS`.

```text
FI-03 = DEFERRED TO M2+
EVIDENCE_CLASS = SYNTHETIC (groundwork only)
```

## M1 Scope Effect

The approved-for-finalization scope makes no M1 claim for external-side-effect
execution or reconciliation. No external provider architecture is added in
this phase, and no new production code is required for this deferment.

## M2+ Follow-up

Future Agent Integration must define:

- external provider execution;
- side-effect reconciliation;
- abrupt interruption recovery; and
- a disposable real-effect harness, interruption points, identity and
  idempotency rules, and retained external evidence.

FI-03 cannot be claimed before that boundary is implemented and evidenced.

## Owner Decision

The Release Owner explicitly accepted this exclusion in the supplied textual
owner declaration. FI-03 remains deferred, not passed, and requires the M2+
follow-up described above before it can be claimed.
