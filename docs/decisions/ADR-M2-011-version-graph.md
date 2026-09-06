# ADR-M2-011: Version Graph

**Status:** Proposed / Internal M2; implementation complete and test-gated
**Date:** 2026-09-04
**Scope:** M2-SLICE-011A contract and M2-SLICE-011B implementation

## Context

M2-SLICE-010 persists an immutable Version as a logical node that references
one immutable Snapshot and one creation Operation. The Version ID is derived
from project, workspace, Snapshot, and creation-operation identity. Version is
not a Snapshot, Operation, commit, branch, or mutable ref, and
`workspaces.head` remains a Snapshot digest.

The 010B predecessor record had no parent relation. 011B adds the bounded
relation without changing the predecessor's identity or operation semantics.

## Problem

A parent relation must have one precise meaning before it is stored. Treating a
parent as creation time, replacement, branch membership, or a Git parent would
conflate independent concepts and make retry, migration, and future branching
ambiguous. The contract must also prevent dangling, cyclic, and cross-scope
relations without changing the identity or operation rules frozen by
M2-SLICE-010.

## Goals

- Define a minimal zero-or-one-parent Version lineage.
- Define root recognition, immutable parent binding, and cycle prevention.
- Preserve the existing Version identity and creation Operation relation.
- Define workspace, project, environment, and current-generation scope.
- Define atomic creation, retry, recovery, and additive migration behavior.
- Bound deletion and CAS retention so graph references cannot dangle.
- Analyze the same-Snapshot/different-operation interaction without silently
  changing the M2-SLICE-010 behavior.

## Non-goals

011B implements the bounded graph without adding branch, merge, rebase,
cherry-pick, commit, Version head, label, candidate, approval, Agent State,
provider, CLI, SDK, or remote synchronization semantics. It does not change M1, ADR-0016, Snapshot
identity, workspace Snapshot-head semantics, or the operation ledger.

## Graph Model

Each Version has zero or one parent Version:

```text
V1 (parent = null)
  <- V2 (parent = V1)
       <- V3 (parent = V2)
```

The relation is directed from child to parent in storage. It forms a rooted
single-parent forest for this slice. A child has at most one parent; the
schema does not make `parent_version_id` unique, so it does not preclude a
future contract from naming multiple children of one Version. Multiple parents
and merge semantics are not defined.

## Parent Semantics

`Vchild.parent_version_id = Vparent.version_id` means:

> Vchild was intentionally created with Vparent as its logical workspace
> lineage base.

It means `based on` / `derived from` at the logical Version level. It does not
mean replacement, mutable succession, branch membership, content delta,
wall-clock predecessor, or Git parent. The relation attests the caller's
declared base; Snapshot bytes remain authoritative for content.

The parent is an immutable binding supplied when the child Version is created.
It is not an input to the already-frozen Version ID digest. Exact retry must
nevertheless supply and validate the same parent. A changed parent for the same
creation Operation is idempotency-key reuse and fails closed. Correcting an
incorrect lineage requires a new creation Operation and a new Version.

## Root Version

A Version with `parent_version_id IS NULL` is a Root Version. No separate
`root_version_id` is stored. Migration must not infer parents from timestamps,
insertion order, Snapshots, or Operations. If a later accepted migration imports
pre-graph Versions, their parent is explicitly `NULL`; multiple roots may
therefore exist in one workspace.

## Immutability

`parent_version_id` is immutable with every other Version field. There is no
assignment or update API. Repeating an assignment is only an exact creation
retry; a different assignment cannot rewrite the row.

## Cycle Prevention

Creation must fail closed unless a non-null parent already exists and its
ancestor chain is valid and terminates at a root. The derived child ID must not
equal the parent ID. Validation must reject repeated ancestor IDs, including
self-parent and longer cycles. Because parent rows pre-exist and parent fields
are immutable, inserting one validated child cannot introduce a cycle into a
valid graph. Cold-open/read validation must still reject persisted corruption
rather than expose a partially trusted graph.

## Workspace Scope

Parent and child must have the same `workspace_id`. Cross-workspace lineage is
invalid even when the Snapshots contain identical bytes. This rule is checked
inside the Version creation transaction and on graph reads.

## Project Scope

Parent and child must have the same `project_id`. Cross-project lineage is
invalid and cannot be enabled by sharing a CAS object or Snapshot digest.

## Generation Scope

The bounded graph implementation requires parent and child to have identical
`environment_id`, `generation_id`, and `migration_id`, and both must satisfy
the active-generation validation already required by M2-SLICE-010. A graph
edge across an environment or generation therefore fails closed.

Whether a future repository migration preserves original generation
attestations, rebinds them, or records an explicit cross-generation lineage is
an OPEN DECISION. No cross-generation edge may be inferred or accepted until a
separate migration contract closes that question.

## Snapshot Relation

Every Version continues to reference exactly one immutable Snapshot. Parent
and child Snapshots are independent content references; the parent relation
does not prove a content delta and does not require different Snapshot IDs.

The parent relation does not require parent and child Snapshots to differ.
Whether different successful creation Operations may create distinct Versions
that reference the same Snapshot remains an OPEN DECISION carried forward from
M2-SLICE-010. The current 010B implementation still fails that case closed with
`CONTRACT_OPEN_DECISION`; 011A does not change that behavior. The design
recommendation is to allow distinct logical Versions for distinct Operations,
because Version identity and auditability include the Operation, but that
recommendation requires a separately accepted contract before implementation.

## Operation Relation

Each Version continues to bind to exactly one `version.create` Operation and
one creation Operation binds to at most one Version. A non-root creation
Operation must include both the Snapshot input reference and the declared
parent Version input reference. A root creation has no parent Version input.
The terminal Operation result references the created Version as today.

Exact retry of the same Operation, Snapshot, and parent returns the same
durable Version. Reusing the Operation with a changed parent, Snapshot, or
scope fails closed. Parent creation does not introduce a new event type or a
second operation ledger.

## Ordering

Identity, lineage, and creation time are distinct:

- `version_id` identifies one immutable Version;
- `parent_version_id` establishes a partial lineage order; and
- `created_at` is query/audit metadata, not an authoritative sequence.

No monotonic Version number or total ordering is introduced. Parent existence
implies only that the parent was durable before the child transaction; callers
must not derive graph order from timestamp ties or lexical IDs.

## Deletion

No Version or Snapshot deletion API exists in the bounded M2 model. Until a
separate retention/GC contract is accepted:

- a Version referenced as a parent must not be deleted;
- a Snapshot referenced by any Version must not be deleted;
- CAS tracing must retain objects reachable through those Snapshots;
- deletion must not cascade from parent to children or Snapshot to Versions;
- an attempted deletion that would create a dangling reference fails closed.

Whether a later system prohibits deletion permanently, uses tombstones, or
collects unreachable history remains an OPEN DECISION. This ADR selects only
the current safe behavior: preserve immutable referenced history.

## Migration

Graph storage is additive in the not-yet-released M2 target schema. The source
v0.1 database is not altered in place and still migrates to an empty `versions`
table with nullable `parent_version_id`; no parent or synthetic Version is
inferred.

Opening an existing same-generation pre-graph `versions` table is an additive
column migration: existing rows receive explicit `NULL` parents and retain
their identity. Importing those rows into a different repository generation is
not defined by M2-SLICE-010; immutable generation/migration attestations cannot
be silently rebound. Cross-generation import must fail closed until a separate
generation-migration contract defines the attestation and compatibility rule.

Migration publication follows the existing selector protocol. The target is
published only after table shape, all Version bindings, parent existence,
scope, and acyclicity validate. An incomplete target remains unpublished.

## Recovery

Version insertion, nullable parent binding, successful Operation outcome, and
Operation journal/event completion are one SQLite transaction. A pre-commit
failure exposes neither the child nor a completed operation. A post-commit
interruption may make the caller outcome unknown, but cold reopen discovers the
complete child and parent binding; exact retry returns that Version. Recovery
must never add, remove, or infer an edge.

## Future Branch/Merge

This single-parent relation is not a branch implementation. A future branch is
expected to be a named/mutable reference governed by its own contract, not a
reinterpretation of `parent_version_id`. Multiple-parent merge nodes, rebase,
and cherry-pick require separate identity, operation, conflict, and recovery
decisions. This schema neither implements nor claims them.

## Alternatives

### Use creation time as the parent

Rejected. `created_at` is not a strict sequence and cannot prove an intentional
base, especially across retries or equal timestamps.

### Treat the previous workspace Snapshot head as the parent

Rejected. `workspaces.head` is a Snapshot digest, not a Version head, and
M2-SLICE-010 intentionally keeps those entities separate.

### Put parent identity in `version_id`

Rejected for this slice. It would revise the deterministic identity frozen by
M2-SLICE-010. Parent remains immutable and retry-validated without changing
the digest inputs.

### Converge Versions that reference the same Snapshot

Not selected in 011A. Allowing distinct Versions is recommended because
convergence discards creation-operation identity and auditability and collapses
a logical node into content identity. The policy remains open, so the current
deterministic rejection remains valid until a later accepted decision.

### Allow multiple parents now

Deferred. It would introduce merge semantics before conflicts, operation
identity, and recovery behavior are defined.

## Closed Decisions

- Parent means an intentional logical lineage base, not replacement, Git
  ancestry, a content delta, or timestamp order.
- Each Version has zero or one immutable parent; null is the root predicate.
- Parent and child have the same workspace, project, environment, generation,
  and migration in the bounded graph.
- Parent existence, scope, and acyclicity validate before atomic insertion.
- Parent is retry-validated but does not change the 010 Version identity digest.
- `created_at` remains non-authoritative query/audit metadata.
- Referenced graph and Snapshot history is not deleted or cascaded in this
  bounded model.
- v0.1 migration creates an empty graph-capable Version table and no synthetic
  lineage.

## Open Decisions

- How Version generation attestations and lineage behave across a future
  repository-generation migration.
- Whether different creation Operations targeting the same Snapshot create
  distinct logical Versions or converge/reject under an accepted policy.
- The long-term deletion policy: permanent retention, tombstones, or tracing
  collection after all graph and Snapshot references are gone.
- Whether a workspace later needs a Version head or named branch refs.
- Multiple-parent merge semantics and any future branch/rebase behavior.
- Whether a separate monotonic logical sequence is needed.
- Human-readable Version labels.

These decisions do not prevent implementing the bounded same-generation,
single-parent relation defined above.

## Implementation Status

M2-SLICE-011B implements this ADR's bounded parent relation in
`src/metadata.rs`. The additive nullable column and index are migration-safe,
parent validation is iterative and fail-closed, and parent/children queries
revalidate durable graph integrity. `tests/version_graph.rs` passes 31 durable
cases on Windows 11 / NTFS local development; the explicit 1,000-node retained
chain sanity also passes but remains development-only and ignored by default.
The existing 010B identity, same-Snapshot open decision, operation ledger,
workspace head, v0.1 empty-table migration, and M1 semantics remain unchanged.
No release evidence, tag, push, public API, branch, merge, or provider was
added.

## Decision

M2-SLICE-011A froze, and M2-SLICE-011B implements, a nullable, immutable,
same-scope `parent_version_id` relation with the semantics and failure
boundaries above. The 011A G1-G15 placeholders remain historical contract
fixtures; the executable 011B integration suite is the implementation
evidence.
