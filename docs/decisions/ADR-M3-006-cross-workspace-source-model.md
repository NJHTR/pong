# ADR-M3-006: Cross-Workspace Immutable Source / Materialization Model

- **Status:** `Proposed / Internal M3`
- **Date:** 2026-09-09
- **Scope:** M3-SLICE-003E contract design only
- **Implementation:** none; no production code, DDL, migration, or runtime API

## Context

M2 binds Version and Snapshot metadata to one Workspace. The CAS manifest and
blobs are immutable content-addressed objects, while `Workspace.head` is a
local Snapshot digest. M3 parallel Executions need to start from another
Workspace's stable Version without weakening those boundaries.

## Existing M2 Invariants

Version ownership, Version identity, same-Workspace parentage, Snapshot
metadata binding, Workspace Head, Version Head, Restore, Diff, Rollback,
Operation identity, and M1 compatibility remain closed. A source row cannot be
reassigned to a different Workspace.

## Current Conflict

Directly setting W2.head to S100[W1] or treating V100[W1] as a W2-local Version
would violate existing integrity checks. A cross-Workspace base therefore
needs a source/target distinction.

## Alternative Models

- **A: Workspace-owned Snapshot plus copy-on-materialize.** Read source CAS and
  publish a target-local Snapshot. Compatible and recommended.
- **B: Project-global immutable Snapshot source.** Changes M2 Snapshot ownership;
  rejected as `CONTRACT_CONFLICT`.
- **C: Global CAS plus Workspace-local Snapshot metadata.** The storage form of
  A; compatible and recommended.
- **D: Independent SourceReference entity.** Deferred unless an accepted
  lifecycle/recovery contract proves one new durable entity necessary; otherwise
  it is unnecessary schema.

## Recommended Model

`MODEL_RECOMMENDATION = A`.

Model C is the compatible storage realization of A: global CAS content with
Workspace-local Snapshot metadata, not a new ownership model or durable entity.

Keep the source immutable and Workspace-owned. A future Execution-level Base
Reference points to the source Version by ID. The target reads and materializes
CAS content, then publishes its own local Snapshot metadata. This preserves the
M2 meaning of every existing field.

## Source Model

Source is `Version[W1] -> Snapshot[W1] -> CAS`. It must be healthy and compatible
by project, environment, generation, migration, redaction, and manifest
integrity. Reading it cannot acquire W1's lease or mutate W1.

## Target Model

Target is the explicit W2 physical tree and W2 durable metadata. Mutation uses
W2's existing lease/revision transaction path. Result Snapshot metadata is
owned by W2 even if it reuses source CAS objects.

## Snapshot Semantics

Source and result Snapshot metadata are distinct. Equal content does not imply
shared Snapshot ownership or shared Workspace Head. CAS bytes remain globally
deduplicated and immutable.

## Workspace Head and Version Head

W2.head points to W2's result Snapshot, never S100[W1]. Version Head stays
Workspace-local and cannot point at V100[W1] under this contract. A local result
Version, if later created, uses valid same-Workspace parent semantics only.

## Version Ownership and Parent

V100 remains W1-owned and immutable. Base Reference is not `parent_version_id`.
Cross-Workspace lineage, Merge, Branch, and Rebase remain future contracts.

## Base Reference

The reference stores only an opaque source Version ID (or an explicitly
equivalent additive Execution attachment). It grants read/materialize/diff
authority, not ownership or write access.

## Diff, Restore, and Parallel Workspace

Cross-Workspace diff is a read-only source-manifest observation. Restore is
target-local materialization with existing operation, lease, revision,
verification, and recovery boundaries. Independent Workspaces may materialize
the same source concurrently; same-Workspace writers conflict deterministically.

## Rollback

Existing rollback remains Workspace-local. A future W2 rollback to V100[W1]
would need to publish a W2-local result Snapshot while preserving the frozen
rollback rule that no Version is created. That combination is not currently
expressible by the closed M2/M3 rollback contract, so cross-Workspace rollback
is `CONTRACT_CONFLICT` until a successor contract explicitly resolves local
Snapshot and Version Head behavior.

## Handoff and Resume

Handoff or Resume may carry the opaque source reference to a new Execution, but
the target independently acquires its Workspace lease and cannot transfer W1
ownership. A failed Execution and its source reference remain durable.

## Failure Boundary, Recovery, and Idempotency

Missing/corrupt source or CAS, incompatible scope, provider/filesystem failure,
stale lease/revision, and persistence failure fail closed. Success requires
verified target materialization plus durable local publication. Reopen resolves
old-or-new or deterministic unknown state using existing operations. Exact
retries return the same durable result; changed requests conflict.

## Migration and Legacy

No ownership rewrite or synthetic legacy reference is allowed. A future
implementation may use one minimal additive Execution-level reference and
existing Operation refs/local Snapshot publication. If more durable entities
are required, stop with `SCHEMA_GAP`. No DDL is performed here.

## Security

Only Version/Snapshot IDs and bounded integrity metadata may cross Workspaces;
secrets, credentials, tokens, prompts, cookies, private keys, and transcripts
are excluded.

## Schema Proposal

No global Snapshot or Version table is proposed. Prefer existing Execution,
Operation, Workspace, Snapshot, Version, CAS, lease, and revision records.

## Open Decisions

Exact base-reference field/relation, local result linkage, cross-Workspace
restore operation naming, cross-Workspace rollback semantics, checkpoint reuse,
and future source-reference lifecycle remain open.

## Risks

Confusing source and result IDs could publish a foreign Workspace Head. A local
Snapshot created by rollback could be mistaken for a new Version. Weak source
validation could expose cross-project content.

## Status

`Proposed / Internal M3`. This ADR closes the source/materialization model only;
it does not implement M3-SLICE-003D or runtime cross-Workspace behavior.
