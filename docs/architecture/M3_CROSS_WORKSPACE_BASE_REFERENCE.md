# M3 Cross-Workspace Base Version Reference

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003C  
**Implementation:** none; no production code, schema, migration, or DDL is authorized.

## Current Conflict

The current M2 model gives every immutable `VersionRecord` one owning
`workspace_id`. Existing Execution creation also validates `base_version_id`
against the Execution Workspace. That is correct for the existing local
lineage model, but it cannot express one stable Version being used as the
read-only starting point for several isolated Workspaces.

The desired parallel shape is:

```text
V100 [owned by W1]
  |\
  | \
 E1/W1  E2/W2  E3/W3
   |      |      |
 V101   V201   V301
```

This proposal resolves the representational gap without changing the meaning
of any existing M2 field.

## Version Ownership

`Version.workspace_id` remains the owning Workspace for the immutable logical
Version. Ownership controls the Version's existing scope and lineage rules;
it is not transferred when another Execution reads the Version as a base.

## Version Content

A Version continues to reference one immutable Snapshot. Snapshot and CAS
content may be shared by multiple Workspaces for read-only materialization.
Sharing content does not share Workspace metadata, lease, revision, head, or
Version Head.

## Base Reference

A **Base Reference** is an immutable Execution-level reference to an existing
Version used only as the starting state for that Execution. It is not Version
ownership, not a Version parent edge, not a Workspace Head, and not a Version
Head.

The reference must validate the target Version as healthy, immutable, and
compatible by project, environment, generation, and migration identity. The
reference carries only the Version identity and bounded integrity facts; it
does not copy secrets, prompts, credentials, or provider state.

The existing M2 `Execution.base_version_id` meaning remains unchanged for
same-Workspace bindings. A future additive implementation may store the
cross-Workspace reference in a separately named field/relation or another
explicit immutable attachment, but must not silently weaken the existing
validator or reinterpret `Version.workspace_id`.

## Execution Workspace

An Execution writes only in its explicitly attached Workspace. A Base Reference
does not authorize writes to the owning Workspace and does not select another
Workspace automatically. The recommended parallel executions are therefore:

```text
E1 -> W1, base reference -> V100[W1]
E2 -> W2, base reference -> V100[W1]
E3 -> W3, base reference -> V100[W1]
```

Each new result Version is owned by its result Workspace. `V201[W2]` and
`V301[W3]` may use V100 as an execution base without making V100 their direct
M2 `parent_version_id`.

## Workspace Head and Version Head

`Workspace.head` remains the Snapshot digest for each physical Workspace.
Materializing S100 into W2 does not update W1.head. Each Workspace has its own
lease, revision, Snapshot Head, and explicit Version Head. Reading V100 as a
base cannot mutate W1's Version Head; selecting a Version Head in W2 remains a
W2-local guarded mutation.

## Snapshot and Materialization

The immutable target Snapshot and CAS objects may be read concurrently by W1,
W2, and W3. Materialization into each destination is independently guarded and
must preserve existing Restore semantics. A source Snapshot/Version is never
modified by another Workspace's materialization, diff, or rollback.

## Version Graph and Version Parent

The existing 011B Version Graph rule remains closed: `parent_version_id` is
same-Workspace immutable lineage. A cross-Workspace Base Reference **must not**
be encoded as `parent_version_id`; doing so would violate the existing
workspace-scope validator. Consequently:

- `V101[W1].parent` may be V100[W1] under the existing rule.
- `V201[W2]` may reference V100[W1] as an Execution Base Reference.
- `V201[W2].parent_version_id` is null or another valid W2 Version according
  to the accepted creation request; it is never inferred from the Base
  Reference.

Cross-Workspace lineage, Merge, Branch, and Rebase remain future contracts.

## Checkpoint

Checkpoint scope remains explicitly tied to its existing Task/Execution/
Workspace contract. A Checkpoint may reference an immutable Version used as a
base, but the reference does not make the Checkpoint globally writable or
transfer its Workspace scope. Whether a Checkpoint may be reused across
Workspaces is an open decision and must be separately authorized.

## Rollback

An Execution in W2 may rollback to the shared V100 Snapshot/Version as a
read-only source. Rollback changes only W2's physical tree, Snapshot Head, and
W2 Version Head under the existing M3 rollback contract. W1, V100 ownership,
W1.head, and W1 Version Head remain unchanged.

## Resume and Handoff

Resume creates a new Execution with an explicit Base Reference; it does not
rewrite the failed Execution or Version graph. Handoff may carry the opaque
Base Reference, but it never transfers Version ownership or a Workspace lease.
The target Execution must independently satisfy project, environment,
generation, migration, lease, and revision checks before writing.

## Lease, Revision, and Isolation

Leases and revisions remain Workspace-local. W1 and W2 can mutate concurrently
with independent CAS and leases. A Base Reference grants no authority over the
owner Workspace. Workspace Head, Version Head, Operation, Checkpoint, Rollback,
and physical tree changes are isolated unless an explicit future coordination
contract says otherwise.

## Failure Boundary and Recovery

Missing, corrupt, cross-project, cross-environment, cross-generation, or
cross-migration base references fail closed before mutation. Provider or
materialization failure exposes the complete old state, complete new state, or
a deterministic unknown/conflict result. Reopen must preserve the immutable
base reference and independently reconcile each Workspace; no phantom base
success or dangling reference is allowed. Exact retry returns the same durable
reference/result.

## Security

Only opaque Version identifiers and redacted integrity metadata cross this
boundary. API keys, OAuth tokens, passwords, cookies, private keys, raw prompts,
transcripts, and provider memory are forbidden.

## Legacy and Migration

M1 v0.1.0 readers and existing M2 tables retain their meaning. Legacy
repositories do not receive synthetic cross-Workspace references. A future
implementation requires an additive migration or relation that leaves all
existing Version rows and `parent_version_id` values unchanged. No DDL is
executed or implied by this proposal.

## Alternatives

### Model A: strict Workspace-local Version only

Safe and already implemented, but requires duplicating V100 for every
Workspace. It supports parallel work only by creating semantically duplicate
baseline Versions and weakens the shared-stable-baseline experience.

### Model B: cross-Workspace Execution Base Reference

**RECOMMENDED.** Keep Version ownership and same-Workspace parentage intact;
add one immutable, read-only Execution-level reference validated by common
scope/integrity identities. It avoids copying content and does not require
Merge or graph rewriting.

### Model C: independent BaseReference durable entity

Potentially useful if references need their own lifecycle, audit, or reuse
outside Executions. It is not required for the default Execution-scoped
contract; introducing it would be an additive schema decision and remains
future. If implementation proves it mandatory, report `SCHEMA_GAP` before
coding.

### Model D: promote Version scope to Project

Rejected for this slice. It would alter M2 Version ownership, parent validation,
Workspace Head/Version Head scope, migration, and legacy compatibility.

## Recommended Model

Adopt Model B as the internal contract proposal:

```text
V100 [Version owner W1, immutable Snapshot S100]
  |-- BaseRef(E1,W1) -> V100 -> V101[W1]
  |-- BaseRef(E2,W2) -> V100 -> V201[W2]
  `-- BaseRef(E3,W3) -> V100 -> V301[W3]
```

The arrows from BaseRef to V100 are not Version parent edges. New Versions
remain owned by their result Workspaces and use only valid same-Workspace
parents under the existing M2 contract.

## Open Decisions

Exact additive storage shape; whether the reference is a field or relation;
Checkpoint reuse across Workspaces; cross-Workspace lineage; base-reference
retention and deletion; Version Head selection policy; cross-project sharing;
automatic conflict resolution; and future Merge/Branch/Rebase.

## Status

`M3-SLICE-003C = CONTRACT_READY` after the companion proposal, ADR, ignored
test contract, and limited validation commands pass. Runtime implementation is
deferred to the next approved slice.
