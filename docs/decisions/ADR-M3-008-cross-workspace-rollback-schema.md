# ADR-M3-008: Cross-Workspace Rollback / Target-Local Snapshot Schema

- **Status:** `Proposed / Internal M3`
- **Date:** 2026-09-09
- **Scope:** M3-SLICE-003H contract design only
- **Implementation:** none; no DDL, migration, production code, or runtime API

## Context

M3-SLICE-003G identified that cross-Workspace rollback needs a durable
distinction between a foreign immutable source and a target-local Snapshot
result. Existing M2/M3 ownership and Head semantics must remain intact.

## Problem

`rollback_records` has a target Version and request status but no typed source
Snapshot, previous target head, result target head, or Version Head before/after
values. Existing completion also assumes the target Version can become the
target Version Head.

## Existing Invariants

Version and Snapshot metadata are Workspace-owned. `Workspace.head` points to a
local Snapshot root digest. Version Head is Workspace-local and independent from
Workspace Head. Rollback does not create a Version. CAS content is immutable and
shareable. Version identity and parentage are immutable.

## Schema Gap

The gap is additive, not a need for a new core entity. Existing fields retained:
`target_version_id`, `target_reference` (typed as Checkpoint ID by
`target_kind`), `result_version_id`, `status`, request/integrity fields, and
timestamps. New nullable typed columns are proposed:

`source_version_id`, `source_snapshot_id`, `previous_workspace_head`,
`result_workspace_head`, `previous_version_head`, and `result_version_head`.

Legacy rows remain readable with NULL additive fields.

## Source / Target Model

Source is `source_version_id -> source_snapshot_id -> CAS`, read-only and owned
by the source Workspace. Target is `workspace_id` plus target-local physical
state and Snapshot metadata. The source Snapshot is never assigned to target
Workspace Head.

## Rollback Record

The record becomes the typed durable authority for source, target, previous
state, result state, status, and exact retry. It does not duplicate an existing
Checkpoint column; `target_reference` remains the existing field.

## Workspace Head

Successful cross-Workspace rollback writes `result_workspace_head`, whose
Snapshot belongs to the target Workspace. A foreign Snapshot cannot become
`Workspace.head`.

## Version Head

**Decision: Model A, unchanged target Version Head.** Cross-Workspace rollback
does not alter the target Workspace's Version Head; `result_version_head`
equals `previous_version_head`. Model B (clear it) loses a valid logical
selection without need. Model C (create/select a target Version) violates the
rollback-no-Version rule. A target can therefore durably have local content
different from the Snapshot selected by its Version Head; M2 explicitly keeps
the two references independent.

## Snapshot Semantics

Rollback may publish a target-local Snapshot without creating a Version. Snapshot
publication and Version creation are separate operations.

## Version Semantics

No Version identity, Workspace ownership, parent edge, or graph lineage changes.
`result_version_id` is NULL. Resume or explicit Version publication may later
create a target-local Version.

## CAS

Source and target may reuse immutable CAS blobs and manifests where valid, but
Snapshot metadata and Workspace Head remain target-local.

## Migration

The six columns are a future additive migration proposal only. Existing rows
are not backfilled or synthesized. No DDL is executed here.

## Recovery

The existing status field can represent prepared/materialized/published,
completed, failed, and unknown recovery boundaries. The new previous/result
columns allow cold reopen to classify old-or-new without JSON parsing.

## Idempotency

The existing request identity and digest remain authoritative. Source, target,
previous head, result head, and expected revision are part of the durable
comparison. Exact retry returns the same record; changed requests conflict.

## Alternatives

JSON packing, a second result entity, foreign Version Head assignment, and
automatic Version creation are rejected. They either weaken typed durability or
change closed M2/M3 semantics.

## Security

Only IDs, digests, and bounded integrity metadata are persisted. No secrets,
tokens, credentials, prompts, memory, or transcripts cross the boundary.

## Open Decisions

Migration version, new-row nullability policy, operation/event linkage,
authorization, and public API exposure remain open.

## Risks

Implementers could confuse source and target Version fields, publish a foreign
Workspace Head, or infer a Version from a target Snapshot. The six typed fields
and Model A Version Head rule are intended to prevent those errors.

## Status

`Proposed / Internal M3`.
