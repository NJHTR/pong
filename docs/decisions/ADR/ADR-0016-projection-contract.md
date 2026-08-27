# ADR-0016: Generation-Bound Projection Contract

- Status: Proposed; core implementation landed, migration evidence and release acceptance pending
- Date: 2026-08-27

## Context

ADR-0008 chooses immutable append-only events as the source of truth and
rebuildable idempotent projections for queryable state. It does not define the
projection identity, state schema, consumer cursor, handler dispatch,
rebuild publication, or generation ordering needed to implement that choice.

The current M1 implementation has an append-only `events` table and operation
journal recovery. The legacy `EventRecord` contains only a per-stream sequence
and a subset of the envelope described by
[`EVENT_MODEL.md`](../../architecture/EVENT_MODEL.md). In particular,
  event type, recorded time, causal links, project-local ordering, and
redaction/capture metadata are not persisted as first-class columns. Existing
`list_events` and migration fixture assertions therefore cannot serve as
projection evidence for PT-13 or FI-10. The implementation now adds an
additive `event_envelopes` source, generation-bound projection state, handler
registry, cursor, idempotent apply, and atomic rebuild; this ADR still governs
the remaining migration and crash evidence.

## Decision

This ADR defines the contract for the internal projection slice. It is not a
public API decision, and release acceptance remains pending until PT-13/FI-10
evidence is complete.

### 1. Immutable source and event envelope

Events are the only authoritative input. A projection MUST NOT rewrite or
delete source events. The additive event schema is reconciled with
`EVENT_MODEL.md` and versioned. The envelope contains:

- `event_id`, `project_id`, `stream_id`, and `event_type`;
- a per-stream `sequence` plus a transactionally allocated project-local
  `project_sequence` (the persisted spelling of the event-model
  `local_sequence`) for cross-stream consumption;
- `schema_version`, `occurred_at`, and `recorded_at`;
- optional actor, workspace, task, operation, causation, correlation, and
  parent-event identifiers;
- redaction status/profile and capture-confidence metadata; and
- canonical payload bytes, payload digest, and integrity metadata.

Unknown additive payload fields remain opaque. An unsupported required or
major schema version fails with `UNSUPPORTED` before mutation.

### 2. Projection identity and state

Every projection has a stable `projection_id`, `projection_schema_version`,
supported source-schema range, `generation_id`, migration identity, and
redaction-profile identity. Its state has a canonical encoding and digest plus
an explicit status: `ready`, `rebuilding`, `degraded`, or `quarantined`.

The durable cursor is part of the same identity boundary. It records at least
the last project-local sequence, source stream/sequence, event ID, and payload
digest, and is bound to the generation and projection schema. A cursor from a
different generation or schema is an integrity error, not a reason to skip
events. Cursor pagination cannot use timestamps or stream-name ordering as a
substitute for the project-local sequence.

### 3. Handler and duplicate semantics

A versioned handler registry dispatches by event type and source schema. Each
handler MUST be deterministic and idempotent. Reapplying the same `event_id`
and payload digest is a no-op; reusing an event ID with different envelope or
payload bytes is `INTEGRITY_ERROR`. Unknown additive event types follow an
explicit policy (preserve-and-skip or `DEGRADED`); they must never be silently
discarded while reporting `ready`.

### 4. Atomic apply and publication

Applying an event, updating the projection state, advancing the cursor, and
recording the state digest occur in one metadata transaction. A rebuild writes
to generation-bound staging state. A crash may expose the previous `ready`
projection or a fully verified replacement, but never a partially applied
state/cursor pair. Source events remain visible and unchanged throughout.

### 5. Rebuild and recovery

A rebuild starts from a clean state or a verified projection snapshot, records
a source high-water mark, replays the complete ordered event range, and
verifies event count, cursor tuple, state digest, and generation identity before
publication. Interruption discards or resumes staging from a durable
checkpoint. Repeating a rebuild, including after a crash, must converge to the
same state and digest. Corrupt staging is quarantined; it is never promoted.

Concurrent event appends require an explicit offline fence or a high-water-mark
reconciliation pass before publication. The implementation must not infer
causal order from wall-clock timestamps.

### 6. Generation migration ordering

Projection state is stored inside the target generation, or in an explicitly
generation-namespaced store whose identity is attested by the target manifest.
Migration cannot replace `repository.json` until metadata and all required
projections pass identity, schema, cursor, integrity, and redaction checks.

Before selector replacement, only the old generation is authoritative. After
replacement, startup validates metadata and every required projection against
the selected generation. A mismatch returns `INTEGRITY_ERROR` or
`RECOVERY_REQUIRED`; it must not fall back to the old generation or expose a
mixed state. Migration journal recovery records projection rebuild status and
remains idempotent.

### 7. Required evidence before acceptance

Acceptance requires versioned projection fixtures, unknown/unsupported schema
vectors, duplicate and conflicting-event tests, cursor ordering tests,
forward/backward migration tests with opaque-field preservation, and a FI-10
crash schedule covering handler, checkpoint, verification, and publication
boundaries. PT-13 must prove semantic preservation rather than only JSON shape
or SQLite table compatibility. Every case needs a cold reopen and source-event
immutability check.

## Consequences

- Projection state becomes a disposable, verifiable read model rather than a
  second source of truth.
- Generation publication remains an old-or-fully-verified-new decision and
  cannot be weakened by a stale projection or cursor.
- The event schema now has an additive, versioned envelope layer. A separate
  fixture and migration review is still required before release acceptance.
- No public Core API, CLI, SDK, runtime, server, or adapter is introduced by
  this ADR.

## Non-goals and current status

This ADR does not introduce a public Core API, CLI, SDK, runtime, server, or
adapter. It does not close the M1 gate. Until an owner accepts this ADR and the
required migration/crash evidence exists, the authoritative status remains:

```text
PT-13: EVIDENCE PENDING
FI-10: EVIDENCE PENDING
M1 Release Gate: NOT PASSED
```

## References

- [`ADR-0008-event-model.md`](ADR-0008-event-model.md)
- [`ADR-0014-repository-generation-migration.md`](ADR-0014-repository-generation-migration.md)
- [`M1_DURABLE_PRIMITIVES_GATE.md`](../../development/M1_DURABLE_PRIMITIVES_GATE.md)
- [`EVENT_MODEL.md`](../../architecture/EVENT_MODEL.md)
