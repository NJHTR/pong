# M3 Operation Ledger Gate

**Status: bounded internal slice frozen until M1 passes; gate not passed.**

This document defines the first M3 implementation boundary. It is a durable
metadata and event primitive, not a runtime interceptor or a public API.

## Current Scope

The Rust Core slice provides:

- a versioned operation envelope associated with project, agent, session,
  workspace, environment, and optional parent-operation identities;
- typed input/output references, resource and before/after state attestations,
  policy data, and explicit reversibility, replayability, and side-effect
  classifications;
- a durable `operations` table with `started`, `completed`, `failed`,
  `cancelled`, and `unknown` lifecycle states;
- a separate `durable`/`unreconciled` recording-quality state;
- canonical, redacted envelope and outcome JSON with an envelope digest;
- request-identity idempotency and operation-identity conflict detection;
- project validation for workspace and environment associations;
- an operation event stream committed in the same SQLite transaction as the
  operation row and the existing `operation_journal` intent/outcome row; and
- before/after SQLite commit failpoints with cold-reopen evidence.

`MetadataStore::open_for_backup` remains a legacy compatibility entrance. It
does not require or create the additive `operations` table. A migration target
initializes and validates the table before selector publication.

## Invariants

1. A successful start acknowledgement has a durable operation row, journal
   intent, and `started` event.
2. A failed transaction leaves no operation row, journal mutation, or event;
   an after-commit error is resolved by cold reopen as durable state.
3. A request identity can be retried only with the same canonical redacted
   envelope. A different envelope is `IDEMPOTENCY_KEY_REUSE`.
4. An operation ID cannot be reused by another request.
5. Only `started` operations can move to one terminal lifecycle state. A
   terminal outcome is immutable and an identical retry is a no-op.
6. Result and error are mutually exclusive: completed carries a result, while
   failed/cancelled/unknown carries an error.
7. Workspace and environment references must exist and belong to the same
   project as the operation.
8. Registered secrets are redacted before persistence and must not occur in
   the SQLite main file, sidecars, journal payloads, or lifecycle events.
9. The `operations` table must be a table with the expected column contract;
   a same-named view or incompatible table fails closed before additive DDL.

## Evidence

`tests/operation_ledger.rs` covers start/finish, typed references, lifecycle
events, request and operation idempotency, same-project bindings, redaction,
cancelled/unknown states, recording quality, and start/finish commit faults.
`tests/compatibility_boundaries.rs` covers the raw v0.1 source fixture, exact
target operations schema/index, source preservation, and target cold reopen.

These are local executable and synthetic fault-injection evidence. They do not
establish runtime capture coverage, provider enforcement, a cross-platform
performance budget, or a release API.

## Explicit Non-goals

- no process/filesystem wrappers or runtime interception;
- no Python SDK, CLI, server, UI, or framework adapter;
- no automatic replay, rollback, or external-effect compensation;
- no task/agent registry beyond identity fields carried by the envelope; and
- no claim that recording an operation proves an external side effect was
  reversible, replayable, or successfully observed.

## Exit Requirements

M3 cannot pass until the supported-platform and old-binary gates inherited from
M1 are accepted, operation schema evolution has compatibility fixtures, all
declared lifecycle and recovery boundaries have host evidence, and the
supported capture matrix is published for any future wrappers. The current
slice is implementation evidence only.
