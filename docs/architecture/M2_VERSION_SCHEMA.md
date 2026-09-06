# M2 Version Schema

**Status:** Proposed / internal M2 contract design
**Scope:** M2-SLICE-010/011 persistence and M2-SLICE-012B Version Head
implementation
**Implementation:** M2-SLICE-010B, M2-SLICE-011B, and M2-SLICE-012B implement
the bounded additive schema and migration behind an internal, test-gated API.
The document remains the contract source; it does not make the Version model
public or change M1 semantics.

## Version Definition

A Version is an immutable durable logical state node. It identifies a logical
version of one workspace by referring to one already-published immutable
Snapshot. A Version is not a filesystem capture, a CAS object, an Operation,
a commit, or a branch.

## Snapshot vs Version

| Entity | Meaning | Owns content bytes? | Mutable? |
| --- | --- | --- | --- |
| Snapshot | Immutable filesystem/content state and canonical tree manifest | CAS tree and file blobs | No |
| Version | Immutable logical node referring to one Snapshot | No; it stores references only | No |

The Snapshot remains the source of truth for manifest, CAS root, file counts,
redaction identity, workspace/project binding, environment binding, and
generation identity. A Version must never copy or replace those content
objects.

## VersionRecord

The implemented M2-SLICE-011B record is:

```text
VersionRecord {
    version_id: String,
    workspace_id: String,
    project_id: String,
    snapshot_id: String,
    creation_operation_id: String,
    environment_id: Option<String>,
    generation_id: String,
    migration_id: String,
    created_at: String,
    parent_version_id: Option<String>,
}
```

There is intentionally no mutable status column. A Version is either absent
or durably present and valid; recovery state belongs to the creating
Operation. The 011B parent field is the only graph relation in this bounded
slice. Branch, merge, candidate, approval, authoring graph, and commit fields
remain out of scope.

M2-SLICE-011B implements this one-field extension in the target schema:

```text
parent_version_id: Option<String>
```

`NULL` identifies a Root Version. A non-null value identifies the one immutable
logical lineage base of the child Version. It is not a Git parent, content
delta, replacement pointer, branch membership, or wall-clock predecessor.

## Table Schema

The implemented 010B additive target-generation design is:

```sql
CREATE TABLE versions (
    version_id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    creation_operation_id TEXT NOT NULL UNIQUE,
    environment_id TEXT,
    generation_id TEXT NOT NULL,
    migration_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX versions_workspace_created
    ON versions(workspace_id, created_at, version_id);

CREATE INDEX versions_snapshot
    ON versions(snapshot_id, created_at, version_id);
```

The base DDL is executed by the additive M2-SLICE-010B migration path and the
011B parent-column migration; the 010A design predecessor itself performed no
migration.

The implemented 011B graph target adds a nullable column and child lookup
index:

```sql
CREATE TABLE versions (
    version_id TEXT PRIMARY KEY NOT NULL,
    workspace_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    creation_operation_id TEXT NOT NULL UNIQUE,
    environment_id TEXT,
    generation_id TEXT NOT NULL,
    migration_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    parent_version_id TEXT,
    CHECK (parent_version_id IS NULL OR parent_version_id <> version_id)
);

CREATE INDEX versions_parent
    ON versions(parent_version_id, version_id);
```

The M2-SLICE-012B workspace target adds the nullable logical selection as an
additive column. SQLite appends the column to legacy tables; reads use explicit
column names/order so the physical column position is not semantic:

```sql
ALTER TABLE workspaces ADD COLUMN version_head_id TEXT;
```

The parent column is non-unique: each child has at most one parent, but the
storage design does not silently prohibit multiple children from naming one
parent. Domain validation remains authoritative for existence, scope, and
cycles. Opening an existing pre-graph table adds the nullable column before
creating the parent index; existing rows become explicit roots.

## Primary Key

`version_id` is the immutable primary key. It is generated as:

```text
ver-<lowercase-hex-sha256(canonical(project_id, workspace_id,
                                    snapshot_id, creation_operation_id))>
```

The canonical input is a version-identity record with an explicit domain
separator and field order. The exact canonical encoding must reuse the
repository canonical-JSON/digest rules when implementation begins. No random
UUID is required. Repeating the same identity inputs therefore derives the
same Version ID before or after a restart.

## Identity

The identity inputs are project, workspace, Snapshot, and creation Operation.
They make a Version unique, durable, retry-discoverable, and independent from
the Snapshot content digest. The creation operation is not itself the Version
ID and the Snapshot ID is not promoted to a Version ID.

## Constraints

- `version_id` is the only primary key.
- `creation_operation_id` is unique, establishing one creation operation to
  one Version.
- `workspace_id`, `project_id`, `snapshot_id`, `generation_id`, and
  `migration_id` are required non-empty values.
- `environment_id` is nullable only to preserve the existing nullable
  Snapshot/workspace environment semantics; when present it must match the
  Snapshot and workspace exactly.
- No uniqueness constraint is placed on `snapshot_id`. The different-
  operation/same-Snapshot policy remains open; 011A recommends distinct logical
  Versions but does not change 010B's deterministic rejection boundary.
- `workspaces.version_head_id` is a nullable explicit Version reference;
  `workspaces.head` remains a Snapshot root digest.
- Implemented `parent_version_id` is nullable, immutable, non-unique, and may
  reference only an existing valid Version in the same workspace, project,
  environment, generation, and migration.

## Indexes

The workspace/created index supports listing a workspace's immutable history;
the Snapshot index supports referential audits and future retention decisions.
Neither index creates ordering authority. `created_at` is query/audit metadata,
not authoritative ordering. The implemented
`versions_parent(parent_version_id, version_id)` index supports deterministic
child lookup without making child order into lineage order.

## Workspace Binding

Creation must load the workspace row in the same transaction and require:

```text
version.workspace_id == workspace.workspace_id
version.project_id   == workspace.project_id
```

The workspace physical locator is not stored in a Version and is not part of
Version identity. The existing Snapshot head is not changed by Version
creation or Version Head selection. Version Head scope validation requires
workspace, project, environment, generation, and migration identity to match.

The additive workspace target stores the nullable logical selection separately:

```sql
ALTER TABLE workspaces ADD COLUMN version_head_id TEXT;
```

Legacy rows receive `NULL`; no Version is inferred or selected during
migration.

## Snapshot Binding

Creation must load `SnapshotRecord` and require that the referenced Snapshot
exists and is internally valid. The Version stores `snapshot_id` only; the
Snapshot remains authoritative for `root_digest`, manifest, CAS objects,
counts, redaction profile, and event linkage. Domain validation must also
verify the Snapshot's workspace/project/environment against the Version input.

## Operation Binding

The `creation_operation_id` must identify an existing operation whose project,
workspace, action, and Snapshot input reference match the Version request. The
operation and Version are related 1:1 for creation: one operation can create
one Version, and exact retry discovers that same row. Operation records remain
the source of truth for intent, lifecycle, failure, and recovery; Version is
the durable successful logical node.

For the graph extension, a non-root creation Operation must carry a
typed input reference to the declared parent Version in addition to its
Snapshot input reference. A root creation carries no parent Version reference.
This reuses the existing operation reference model and adds no event type or
parallel ledger.

## Generation Binding

`generation_id` and `migration_id` are copied as immutable attestations from
the opened repository/Snapshot identity. They are split into the same fields
used by existing metadata rather than introducing a new opaque generation
format. Creation must reject a Snapshot whose generation/migration identity
does not match the active repository.

For the bounded graph, a non-null parent must also have exactly the child's
`environment_id`, `generation_id`, and `migration_id`. Cross-environment and
cross-generation edges fail closed. How graph attestations survive a future
repository-generation migration remains an explicit future migration decision;
no cross-generation edge may be inferred meanwhile.

## Parent Binding

The implemented `parent_version_id` has the following contract:

- `NULL` means Root Version; no separate root identifier is stored.
- A non-null parent must already exist and be healthy before child insertion.
- Parent and child must share workspace, project, environment, generation, and
  migration identity.
- The parent and every ancestor must be valid and the chain must terminate at
  a root without repeating a Version ID.
- Parent binding is immutable and cannot be assigned after creation.
- Parent is not added to the frozen Version-ID digest. Exact retry still must
  present the same parent; a changed parent is idempotency-key reuse.

Parent means the declared logical workspace lineage base. It neither verifies
a filesystem delta nor implies replacement, temporal adjacency, or branch
membership.

## Root Version

`parent_version_id IS NULL` is the complete root predicate. Migration does not
infer lineage from `created_at`, row order, Snapshot identity, or Operation
history. If a later accepted migration imports pre-graph Version rows, they
receive explicit `NULL` parents. Consequently one workspace may contain
multiple roots.

## Immutability

After insertion, these fields are never updated or replaced:

```text
version_id
workspace_id
project_id
snapshot_id
creation_operation_id
environment_id
generation_id
migration_id
created_at
parent_version_id
```

If a different Snapshot or binding is needed, a new Version and a new
creation operation are required. There is no Version update API in this slice.

## Referential Integrity

The database-level relationship is a logical reference to
`snapshots(snapshot_id)`. A SQLite foreign key may be added only if it is
compatible with the existing additive-generation migration machinery; domain
correctness must not rely on that FK alone because the current schema does not
use a complete FK graph for workspaces, snapshots, and operations.

Domain validation must prove all of the following before insertion and on
read/audit:

- Snapshot row exists;
- Snapshot manifest and CAS root verify;
- Snapshot workspace and project equal the Version binding;
- Snapshot environment equals the Version/workspace environment, including
  the explicit null case;
- Snapshot generation and migration identity equal the active repository;
- Snapshot operation/event linkage remains valid; and
- the Version row's immutable fields have not been changed.

For graph-capable rows, validation must additionally prove:

- non-null parent row exists and is healthy;
- parent scope and environment/generation identity match the child;
- self-parent and repeated ancestor IDs are absent; and
- the ancestor chain terminates at a root.

A corrupted or missing Snapshot must make the Version unhealthy and fail
closed. It must never be reported as a healthy Version merely because its row
exists.

## Transaction Boundary

The minimum creation transaction is:

```text
validate active generation and Snapshot/CAS/domain bindings
  -> validate existing creation Operation identity
  -> validate parent existence, scope, and acyclic ancestry
  -> insert Version and parent binding (or recognize exact existing row)
  -> record the Operation's successful terminal result/reference
  -> commit
```

The operation and Version must become visible together. A pre-commit failure
exposes neither new Version nor completed operation. A post-commit interruption
may return an unknown caller result, but cold reopen must find both durable
records and exact retry must return the same Version. A permanently completed
operation without a Version is not an accepted steady state.

## Retry Semantics

- Exact operation and identity retry returns the original `version_id` and
  durable record.
- Same operation with a different Snapshot, workspace, project, or generation
  fails deterministically as idempotency-key reuse or integrity failure.
- Same operation with a different parent, including null versus non-null, fails
  deterministically as idempotency-key reuse.
- A supplied/deduced identity that conflicts with an existing `version_id`
  fails closed; it never rewrites the row.
- Different operations targeting the same Snapshot remain an OPEN DECISION.
  The current implementation rejects this case deterministically; 011A
  recommends distinct logical Versions but does not make that production
  behavior normative.

## Snapshot Deletion

The current M2 slices do not implement Version deletion, Snapshot deletion, or
CAS garbage collection. Until a separate retention contract is accepted, a
parent Version and any Snapshot referenced by a Version must be retained.
Cascade deletion is prohibited and any operation that would leave a dangling
parent or Snapshot reference must fail closed. Future permanent retention,
tombstone, and tracing-collection policy remains an OPEN DECISION; no deletion
behavior is implemented here.

## Migration

Migration is additive and uses the existing generation selector protocol:

1. Open the v0.1 source through the read-only backup path.
2. Create the target generation with the `versions` table and its indexes.
3. Copy existing M1/M2 rows without changing their meaning; start `versions`
   empty because v0.1 has no Version entity.
4. Validate table shape, repository identity, redaction profile, and selector
   manifest.
5. Publish the target selector atomically only after all validation passes.

The source v0.1 database is never altered by adding the table. A legacy
repository may be opened, read, and migrated with an empty Version table.

The graph field is added to the not-yet-released M2 target schema. A raw v0.1
migration still starts `versions` empty. Opening a non-empty pre-graph
`versions` table is an additive local schema migration: the nullable column is
added, existing rows receive `NULL`, and the parent index is created only after
the column exists. A separate generation-migration contract is still required
for importing such rows across generations; no cross-generation edge is
inferred.

## Recovery

An interruption before target table creation leaves the source generation
visible. An interruption after table creation, indexes, or constraints but
before selector publication leaves the source visible and the incomplete
target unpublished; retry rebuilds or discards that target according to the
existing selector journal. Cold reopen accepts only a complete old or complete
new generation, never a mixed state. No Version is inferred from an Operation
result during recovery.

For graph creation, Version insertion, parent binding, Operation completion,
journal update, and Operation event append are one SQLite transaction. A
post-commit unknown outcome is recovered by reading the immutable parent
binding; retry never adds, removes, or changes an edge.

## Compatibility

M1 `v0.1.0` table meanings, Event Envelope semantics, projection semantics,
operation ledger semantics, workspace Snapshot head semantics, and old-reader
behavior remain unchanged. The additive `versions` table is invisible to a
legacy reader. A legacy repository with no Version rows is valid and is not
backfilled with synthetic Versions.

## Future Extensions

Branches, merge/rebase, candidate/review/approval, ancestor/descendant
traversal, cross-generation lineage, deletion, and public APIs require separate
bounded contracts. Version Head selection is implemented only as the bounded
workspace-local `version_head_id` reference described by M2-SLICE-012B.

## Version Head Contract

`workspaces.version_head_id` is nullable. `NULL` means that the workspace has
no selected logical Version. A non-null value must identify one healthy,
durable Version in the same workspace, project, environment, generation, and
migration scope. The reference is independent from `workspaces.head`, which
continues to store a Snapshot root digest.

`get_current_version(workspace_id)` is read-only and fails closed on a missing,
corrupt, or out-of-scope reference. `set_version_head` validates the target,
lease, and expected revision, then updates `version_head_id` and increments
the workspace revision in one SQLite transaction. It emits no operation or
event. Exact retries recognize the requested value at the immediately
preceding revision; stale revisions and leases return the existing conflict
error. Before-commit faults roll back both fields; post-commit uncertainty is
resolved by cold reopen and retry.

Version creation does not implicitly select its new Version. A legacy v0.1
workspace migrates with `version_head_id = NULL`, even when Version rows exist
in a later generation; explicit selection is required.

## Query Contract

The bounded internal read API is:

```text
get_version(version_id) -> Option<VersionRecord>
get_parent(version_id) -> Option<VersionRecord>
get_children(version_id) -> Vec<VersionRecord>
get_current_version(workspace_id) -> Option<VersionRecord>
```

For a valid root, `get_parent` returns `None`; a missing referenced parent is an
integrity error, not a root. Children are returned deterministically by
`version_id`; that order has no lineage authority. These queries validate
scope and graph integrity and do not perform general ancestor/descendant
traversal.

## CLOSED DECISIONS

- Snapshot and Version are distinct entities.
- Version references an existing Snapshot; it does not copy content.
- Version identity is durable, deterministic, and immutable.
- One creation operation binds to at most one Version.
- Version storage is additive and does not alter M1 schema meaning.
- Workspace `head` remains a Snapshot head in this slice.
- Production implementation and migration are not started in 010A.
- Parent means a declared logical `based on` / `derived from` lineage, not Git
  ancestry, replacement, or timestamp order.
- Each child has zero or one immutable parent; null identifies a root.
- Parent and child are restricted to the same workspace, project, environment,
  generation, and migration in the bounded graph.
- Version identity inputs remain unchanged; parent is immutable retry-validated
  data, not a new digest input.
- `created_at` is query/audit metadata, not an authoritative Version sequence.
- Referenced parent Versions, Snapshots, and reachable CAS objects are retained
  until a separate deletion/GC contract exists; cascade deletion is forbidden.
- Graph storage is additive for the unreleased M2 target; v0.1 produces an
  empty graph and no inferred roots.
- Version Head is an explicit nullable workspace reference to a durable Version;
  it does not reuse `Workspace.head`, and v0.1 migration leaves it `NULL`.
- `get_current_version` validates the referenced Version, Snapshot, operation,
  parent chain, and workspace scope; broken references fail closed without
  repair.
- `set_version_head` uses the existing lease/revision CAS and commits the head
  and revision atomically without creating an operation or event.

## OPEN DECISIONS

- Long-term Version/Snapshot deletion, tombstoning, and CAS tracing policy after
  graph references are considered.
- Whether different creation Operations targeting the same Snapshot create
  distinct logical Versions or converge/reject under an accepted policy.
- Future Version Head scopes beyond one workspace and their authorization.
- How Version attestations and lineage behave across a future repository-
  generation migration.
- How a non-empty pre-graph Version table is migrated without rewriting its
  immutable generation/migration attestations.
- Multiple-parent merge, branch, rebase, and graph traversal semantics.
- Whether a separate monotonic logical sequence is needed.
- Human-readable Version labels.
