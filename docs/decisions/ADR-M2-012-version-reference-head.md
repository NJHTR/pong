# ADR-M2-012: Version Reference / Head Contract

**Status:** Proposed / Internal M2
**Date:** 2026-09-05
**Scope:** M2-SLICE-012A architecture contract only
**Implementation:** M2-SLICE-012B implemented as `INTERNAL / TEST-GATED`; the
bounded workspace-local nullable `version_head_id` proposal is retained, and
all open decisions below remain unresolved.

## Context

M2 already has two durable concepts with different meanings. A Snapshot is an
immutable content state and `WorkspaceRecord.head` stores its root digest. A
Version is an immutable logical node that references one Snapshot and may have
one immutable parent. M2-SLICE-011B closes the single-parent graph contract but
does not add a current Version reference.

The next slice may need to answer which logical Version a workspace is
currently using. That answer must not be inferred from a content digest,
creation time, parent order, or the last row inserted into `versions`.

## Current Snapshot Head

`WorkspaceRecord.head` remains the current Snapshot head. Its value is an
optional Snapshot root digest, and existing snapshot publication, status, diff,
restore, event, projection, and M1 compatibility semantics continue to use it
with that meaning. Version creation does not update this field.

## Version Head Problem

`V1 -> S1` and `V2 -> S2` together with `Workspace.head = S2` prove only that
the workspace content head is `S2`. They do not prove that `V2` is the selected
logical Version: more than one Version may eventually refer to the same
Snapshot, and a logical selection may be detached from the latest content
publication. A Version Head, if introduced, therefore needs its own explicit
reference and integrity contract.

## Snapshot Head vs Version Head

The concepts are separate:

| Reference | Meaning | Current state |
| --- | --- | --- |
| Snapshot Head | Current immutable content state of the workspace | `WorkspaceRecord.head`; implemented |
| Version Head | Explicitly selected immutable logical Version | No field or API; proposal only |

No implementation may reinterpret `WorkspaceRecord.head` as a Version ID or
silently derive a Version Head from it.

## Version Selection

The selection operation, if accepted, must persist an explicit Version
reference and must validate that the referenced Version is durable, healthy,
and bound to the selected scope. Selection is not Version mutation: Version
identity, Snapshot reference, parent, and creation metadata remain immutable.

This ADR does not choose whether the reference is stored on Workspace, in a
separate logical-reference entity, or at another bounded scope. It records the
invariant that a current selection cannot be inferred from graph order.

## Same Snapshot Different Operation

The existing 010B behavior remains unchanged: different creation Operations
targeting one Snapshot are rejected deterministically as
`CONTRACT_OPEN_DECISION`. The Version Head question does not authorize a new
convergence rule. Before any future policy is accepted, it must account for:

- operation identity and exact retry;
- auditability of distinct logical interpretations;
- whether logical history can contain multiple nodes for one content state;
- detached selections and future branch/agent workflows; and
- recovery when a selection points at one of several valid Versions.

## Parent vs Head

`parent_version_id` is historical lineage: the declared logical base of a
Version. A Version Head is current selection. They are not interchangeable. A
valid graph may have `V1 -> V2 -> V3` while a future explicit Version Head
selects `V2`; that selection must not rewrite, remove, or infer graph edges.

## Detached Version

Versions that remain durable but are not selected are valid historical records.
With a future Version Head at `V3`, `V1` and `V2` remain detached historical
Versions unless a separately accepted retention contract says otherwise.
Detachment does not change identity or parentage and does not imply deletion.

## Workspace Scope

If a Version Head is introduced, the default candidate scope is one
`workspace_id`, because Snapshot Head and lifecycle lease semantics are already
workspace-scoped. This is a proposal, not a frozen decision. A project-wide or
global reference must not be introduced implicitly; any different scope needs
its own identity, authorization, and recovery contract.

## Generation Scope

Any future Version Head must point only to a Version whose
`workspace_id`, `project_id`, `environment_id`, `generation_id`, and
`migration_id` satisfy the same bounded graph rules as 011B. Cross-generation,
cross-migration, and cross-environment selection is rejected until a separate
generation migration contract defines how immutable attestations transfer.

## Version Number

No ordinal or monotonic human-facing Version number is added in this slice.
`version_id`, `parent_version_id`, and `created_at` remain distinct; creation
time and lexical ID order are not a sequence. A future ordinal would require
scope, allocation, retry, migration, and deletion decisions.

## Human Label

Human-readable labels are not implemented or required. If labels are accepted
later, they must be mutable descriptive metadata or a separately versioned
record, and must not be an input to the existing deterministic `version_id`.
Changing a label must never change Version identity, parentage, Snapshot
binding, or retry behavior.

## Transaction

If Version creation also selects a Version Head in a future contract, the
Version row, selection reference, operation terminal result, and any journal
state must become visible atomically. The transaction must validate the
Snapshot, Version, scope, generation, and parent graph before writing the
reference. A head must never commit a pointer to a missing or uncommitted
Version.

If selection is a separate operation, it still needs a durable operation
identity and lease/revision guard appropriate to its accepted scope. No new
event taxonomy is implied by this proposal.

## Recovery

After a pre-commit failure, neither a new Version Head nor a completed
selection operation may be visible. After an unknown post-commit outcome, cold
reopen must find either the old complete reference or the new complete
reference, never a dangling or partially written one. Retry must recognize the
same durable result deterministically and must not increment, rewrite, or move
the reference twice.

## Deletion

No Version or Snapshot deletion API exists in the current bounded model. A
future Version Head would add another retention constraint: the selected
Version cannot be deleted, and deleting a parent or referenced Snapshot must
fail closed while descendants or heads depend on it. Cascade deletion is not
permitted. Permanent retention, tombstones, and CAS tracing remain a separate
open contract.

## Compatibility

M1 `v0.1.0` meanings remain unchanged. In particular, `WorkspaceRecord.head`
continues to be a Snapshot root digest, legacy readers do not need to
understand a Version Head, and v0.1 migration does not infer or synthesize
Versions or logical selections. Existing operation, event, projection,
workspace lifecycle, and 011B parent semantics remain closed.

## Alternatives

### Derive Version Head from `Workspace.head`

Rejected. A Snapshot digest is not a Version identity, and multiple logical
Versions or detached history make derivation ambiguous.

### Use graph order or latest `created_at`

Rejected. Parentage is a partial lineage relation and `created_at` is audit
metadata, not an authoritative current-selection order.

### Store a mutable name in the existing generic refs table

Deferred. A current logical reference needs explicit Version validation,
scope, transaction, recovery, and deletion semantics; reusing `refs` without
that contract would make integrity accidental.

### Add `workspace.version_head` immediately

Deferred. This is a plausible future design, but it would be a schema and
durability commitment before scope, selection, and migration decisions are
accepted.

## Open Decisions

- Whether to introduce a Version Head at all.
- Whether a Version Head belongs to Workspace or another bounded scope.
- Whether multiple Versions may refer to one Snapshot and how selection chooses
  among them.
- Whether selection is a distinct operation or is coupled to Version creation.
- Whether a human-readable label is allowed and how it is retained.
- Whether an ordinal/Version number is needed.
- How Version Head interacts with deletion, tombstones, and CAS GC.
- How a reference behaves across repository-generation migration.

## Contract Test Status

`tests/version_reference_contract.rs` contains R1-R15 placeholders. Every test
is ignored with `NOT_IMPLEMENTED_CONTRACT_TEST`; none is implementation
evidence and none counts as PASS.

## Decision

This document keeps the Snapshot Head and a possible Version Head explicitly
separate and leaves the listed product and schema choices open. It authorizes
no production implementation, DDL, migration, public API, branch, merge,
candidate, approval, or agent-state behavior.
