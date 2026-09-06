# ADR-M2-010: Version Persistence

**Status:** Proposed / Internal M2
**Date:** 2026-09-04
**Scope:** M2-SLICE-010A contract and schema design only

## Context

M2 already persists immutable filesystem Snapshots, workspace/project and
environment bindings, generation identity, and durable operation identity.
M2-SLICE-010 needs a minimal logical Version node without conflating that node
with a Snapshot or an Operation.

## Problem

The current schema has no independent Version entity. `snapshots` describes
immutable content, `operations` describes attempted work, and `refs` stores
generic named values. None of those rows can independently guarantee a
Version's immutable identity, Snapshot reference, creation-operation binding,
or cold-reopen referential integrity.

## Goals

- Define an immutable durable Version node referring to one existing Snapshot.
- Bind Version creation to one existing durable Operation identity.
- Preserve workspace, project, environment, generation, and migration
  integrity checks.
- Make exact retry deterministic without random ID allocation.
- Provide an additive migration design that leaves M1 meaning unchanged.
- Specify recovery and compatibility behavior before implementation.

## Non-goals

This ADR does not implement a `versions` table, migration, Version API,
Version graph, parent relation, branch, merge, rebase, candidate, review,
approval, Version head, public CLI/SDK, provider, or remote synchronization.
It does not modify M1, ADR-0016, the operation ledger, workspace head, or
SQLite initialization in 010A.

## Alternatives

### 1. Independent `versions` table

An additive table gives Version its own primary key, immutable fields, unique
creation-operation binding, indexes, and domain integrity boundary while
retaining Snapshot and Operation as separate sources of truth. This is the
selected design for implementation in a later slice.

### 2. Store Version in `operations.result`

This preserves the operation row but does not create an independently
queryable or immutable Version entity. Result JSON can be rewritten under
future operation semantics and cannot provide a Version-specific uniqueness
or referential-integrity boundary.

### 3. Store Version in `refs`

The generic `refs(name, value, updated_at)` table is mutable named state. It
has no Version record shape, immutable field constraints, Snapshot binding
checks, or operation-to-Version uniqueness. Using it would turn a logical
Version into a mutable ref and pre-empt branch semantics.

### 4. Treat Snapshot as Version

This violates ADR-0006 and the M2 model: an immutable content capture is not a
logical version node. It also prevents multiple logical interpretations and
future parent/branch relationships without changing Snapshot identity.

## Decision

Use an additive `versions` table in a later implementation slice. The minimum
record stores `version_id`, workspace/project binding, `snapshot_id`,
`creation_operation_id`, optional environment identity, generation/migration
identity, and `created_at`. It does not copy CAS/tree data and does not add a
workspace Version head.

`version_id` is derived deterministically from canonical project, workspace,
Snapshot, and creation-operation identity inputs using the repository digest
rules. `creation_operation_id` is unique, so one operation cannot create two
Versions. Exact retry returns the same row; changed operation semantics fail
closed. Different operations targeting one Snapshot remain an explicit open
decision rather than an accidental uniqueness rule.

Creation must validate Snapshot existence, manifest/CAS integrity, workspace,
project, environment, generation, migration, and operation identity in one
transaction. Migration uses the existing generation selector protocol and
starts the additive table empty for v0.1 repositories.

## Consequences

Version becomes independently durable and auditable without duplicating
content or changing the M1 workspace Snapshot head. The implementation will
need additive schema validation, a Version record API, transaction failpoints,
and cold-reopen tests. Snapshot deletion and Version head behavior remain open
and cannot be inferred by implementation.

## Implementation Status

`M2-SLICE-010A = CONTRACT_READY` after the schema document and test contract
were reviewed. `M2-SLICE-010B = PASS / INTERNAL / TEST-GATED` implements the
bounded persistence described here. ADR-M2-010 remains Proposed / Internal M2;
no public API or later Version/Branch/Merge semantics are accepted by this
record.
