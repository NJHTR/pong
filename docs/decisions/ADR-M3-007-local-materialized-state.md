# ADR-M3-007: Local Materialized State / Version Decoupling

- **Status:** `Proposed / Internal M3`
- **Date:** 2026-09-09
- **Scope:** M3-SLICE-003F contract design only
- **Implementation:** none; no production code, DDL, migration, or runtime API

## Context

M3 needs an Execution in W2 to consume an immutable Version/Snapshot owned by
W1. M2 correctly rejects assigning W1 Snapshot metadata to W2 Workspace Head.
The missing distinction is between an immutable source, target Working State,
and a target-local Snapshot publication.

## Existing M2 Invariants

Version and Snapshot metadata remain Workspace-owned. CAS content is immutable
and shareable. `Workspace.head` is a local Snapshot content head;
`Workspace.version_head_id` is an independent explicit Version selection.
Version identity, same-Workspace parentage, Restore, Diff, Rollback, Operation
identity, M1 v0.1.0, and ADR-0016 remain unchanged.

## Decision

Adopt `SOURCE -> WORKING STATE -> TARGET SNAPSHOT` as the only compatible
cross-Workspace materialization model. A target Snapshot is allowed without a
Version. It sets the target Workspace Head and leaves the target Version Head
unchanged or null. A later explicit local Version publication may select that
Snapshot.

This is not a change to M2: M2 already keeps the two head references
independent. It is the explicit contract interpretation required to avoid
confusing a content publication with a logical Version publication.

## Alternative Models

- Reassign the source Snapshot to W2: rejected as `CONTRACT_CONFLICT`.
- Make Snapshot/Version project-global: rejected as `CONTRACT_CONFLICT`.
- Treat every materialization as a new Version: rejected because it changes
  Version creation and rollback semantics.
- Add a separate WorkingSnapshot core entity: unnecessary for the current
  independent head fields; defer and report `SCHEMA_GAP` only if future retry or
  recovery requirements prove it mandatory.

## Source Model

Source is immutable `Version[W1] -> Snapshot[W1] -> CAS`. It is validated for
existence, health, project, environment, generation, migration, manifest, and
CAS integrity. It grants no source Workspace lease or write authority.

## Target Model

Target is the explicitly authorized W2 tree and W2-local Snapshot metadata.
Materialization uses W2 lease/revision and existing transaction/recovery paths.

## Snapshot Semantics

Source and target Snapshot metadata are distinct. Because the canonical tree
manifest includes Workspace identity and the root digest is globally unique,
target materialization normally produces a target-specific root digest even
when file blobs are identical. CAS file bytes may still be reused. Target
publication is immutable local metadata and does not create a Version
automatically.

## Workspace Head and Version Head

After materialization, `W2.head = S200[W2]`. `W2.version_head_id` remains its
prior valid W2 Version or `NULL`. It must never become `V100[W1]`. The resulting
state is durable and explainable, not transient corruption.

## Version Ownership and Parent

Versions remain Workspace-owned. A later W2 Version uses a valid W2 parent or
null. Source references never become `parent_version_id` edges.

## Base Reference

The Execution stores only an opaque source Version ID or equivalent explicitly
named additive attachment. It is read-only and distinct from target Snapshot,
Workspace Head, Version Head, and Version parentage.

## Diff and Restore

Normal diff uses the target Workspace Head; explicit source diff uses the source
manifest. Both are read-only. Cross-Workspace restore materializes source into
the target and publishes target-local Snapshot metadata/head while preserving
the source row and target Version Head.

## Rollback

Existing local rollback remains unchanged and creates no Version. A future
cross-Workspace rollback may use the same source/target materialization flow,
publish a target-local Snapshot, leave target Version Head unchanged or null,
and keep `result_version_id = NULL`. It cannot assign a foreign Version Head.
Until its authorization and operation identity are implemented, the runtime
must reject it deterministically; this ADR does not implement or silently
expand 002C/002D.

## Resume and Handoff

Resume and Handoff carry the source Version reference to a new Execution. A
target Snapshot is not a base Version. The new Execution obtains its own target
Workspace lease; source ownership never transfers.

## Parallel Workspace

Independent targets can concurrently materialize, diff, restore, or (when the
future variant is accepted) rollback from one source. Each target has isolated
heads, lease, revision, operation, and recovery state.

## Failure Boundary, Recovery, and Idempotency

Missing/corrupt/incompatible source, missing CAS, filesystem/provider failure,
stale lease/revision, verification failure, and persistence failure fail closed.
Only a verified target tree plus durable local Snapshot publication is success.
Reopen resolves old-or-new or deterministic unknown state. Exact retries return
the same local result; changed requests conflict.

## Migration and Legacy

No schema, DDL, ownership rewrite, or synthetic legacy reference is proposed.
Legacy v0.1 readers retain existing behavior. Prefer one minimal additive
Execution-level source reference plus existing Operation refs and Snapshot
publication.

## Security

Only opaque IDs and bounded integrity metadata cross the boundary. Secrets,
credentials, tokens, prompts, cookies, private keys, and transcripts are not
copied.

## Open Decisions

Cross-Workspace materialization operation naming; target result linkage;
cross-Workspace rollback authorization; checkpoint reuse; reference retention;
and future public API exposure remain open.

## Risks

Confusing target Snapshot with Version would create false lineage. Assigning a
foreign Version Head would violate Workspace ownership. An incomplete physical
materialization could be reported as a durable head without reopen
reconciliation.

## Status

`Proposed / Internal M3`. This ADR closes the local materialized-state model
only; it does not implement runtime materialization.
