# M3 Parallel Agent Workspace Contract

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003A  
**Implementation:** none; this document authorizes no runtime, schema, or provider change.

## Boundary

This proposal defines ownership and concurrency semantics for multiple Agent
Executions that work on one Task. It composes the existing M2 Workspace,
Snapshot, Restore, Diff, lease, revision, Version Graph, Version Head, and
M3 Agent/Task/Execution contracts. It does not implement an orchestrator or
change any existing identity.

The M2/M3 authorities remain unchanged:

- `Workspace.head` is the current Snapshot root digest.
- `Workspace.version_head_id` is a separate explicit Version selection.
- Snapshot, Version, Operation, Execution, and parent Version identities are
  immutable under their existing contracts.
- Workspace mutations use the existing lease and revision CAS.
- A provider descriptor is metadata, not Core authority.

## Ownership Models

### Mode A: one Execution -> one Workspace

This is the safest ownership shape. One writable Execution is attached to one
explicit Workspace and may mutate it only while holding that Workspace lease.
The attachment and lease are independent checks; graph membership alone grants
no authority.

### Mode B: multiple Executions -> one Workspace

This is **not allowed by default**. A future explicit collaboration mode may
permit it, but every mutation must still serialize through the existing lease
and revision guards. A second writer receives a deterministic Workspace, Lease,
or Revision Conflict. Last-write-wins and implicit writer sharing are
forbidden. This slice does not implement shared writers.

### Mode C: one Task -> multiple Workspaces

This is the **RECOMMENDED** parallel model. Executions may share a Task and an
explicit `base_version_id`, while each writable Execution receives its own
Workspace and lease. E1/W1, E2/W2, and E3/W3 therefore produce independent
heads, Operations, and Version children. The recommendation is a contract
decision, not an automatic scheduler or runtime rule in this slice.

## Authority Matrix

| Action | Required scope | Parallel behavior |
| --- | --- | --- |
| create/attach | explicit Task/Execution/Workspace scope | independent Workspaces may proceed |
| snapshot | attached Workspace + valid lease + revision | W1 and W2 may publish concurrently |
| restore | target Workspace + valid lease + revision | independent targets may proceed |
| rollback | explicit Execution scope + Workspace lease/revision | isolated targets may proceed |
| status | Workspace scope and readable durable state | read-only; no lease mutation |
| diff | point-in-time Workspace observation | read-only; M2 reconciliation applies |
| Version Head selection | explicit Workspace/logical scope + lease/revision | competing selections conflict deterministically |
| handoff | same Task/project and explicit authorization | does not transfer a lease implicitly |
| resume | new Execution identity and explicit base | siblings remain unaffected |

Reconciliation is the observer of instability. It consumes the authoritative
before/after fields and classifies a conflict; it does not silently resolve a
concurrent writer.

## Parallel Execution and Versions

Multiple Executions may legally share `base_version_id = V100`. Their outputs
are independent children, for example `V101.parent = V100`,
`V201.parent = V100`, and `V301.parent = V100`. The existing Version Graph
already expresses this shape; no Merge, Branch, Rebase, or graph rewrite is
introduced.

Each Workspace owns an independent Snapshot Head. Publishing S101 on W1 does
not modify W2.head or W3.head. A successful Version creation does not infer a
Version Head update. Version Head selection is explicit, lease/revision guarded,
and conflict checked. Two concurrent requests selecting different heads do not
use completion order or last-write-wins; one request is rejected with a
deterministic Version Head Conflict and the caller must reconcile.

## Lease and Revision

The Workspace lease belongs to the Workspace, not to a provider or Task. The
current lease holder and epoch must match every mutating request. Handoff does
not transfer, renew, or forge a lease. A target Execution must acquire and
revalidate the lease before it writes. Independent Workspaces may hold leases
in parallel; one Workspace has at most one default writable holder.

Every mutation carries the expected Workspace revision. A changed revision is
a deterministic Revision Conflict, even when the caller's intended content is
otherwise equivalent. Reads do not advance revision, lease, Version Head,
Operation, or event state.

## Concurrency Matrix

- Concurrent snapshots on W1/W2 use independent CAS, metadata, heads, and
  Operations and may both succeed.
- Concurrent diffs are independent read-only observations. They do not append
  operations/events or renew leases.
- Concurrent restores or rollbacks on unrelated Workspaces may proceed. The
  same Workspace requires lease/revision serialization and otherwise returns a
  deterministic conflict.
- Resume creates a new Execution. It does not overwrite the failed or
  interrupted Execution and does not affect sibling Workspaces.
- A concurrent mutation that races a scan is classified using M2's
  `UNSTABLE_OBSERVATION`/Conflict result; stale output is never reported as
  stable.

## Conflict Model

The following classes are stable domain outcomes, not provider strings:

1. **Workspace Conflict:** two writers target one Workspace without an
   accepted collaboration mode.
2. **Revision Conflict:** expected revision differs from durable revision.
3. **Lease Conflict:** owner, epoch, or expiry is not current.
4. **Version Head Conflict:** concurrent logical head selections disagree.
5. **Handoff Conflict:** competing takeover requests target the same
   Execution/request boundary.
6. **Rollback Conflict:** rollback races another mutation on the same scope.
7. **Reconciliation Conflict:** authoritative fields changed during an
   observation.

All conflicts are deterministic and fail closed. No implicit retry converts a
different request into success.

## Handoff, Checkpoint, Rollback, Resume

Handoff preserves Task, Execution history, Version references, Operation
identity, and Workspace identity. It links explicit source and target
Executions and requires same Task/project authorization. A target must acquire
the applicable lease; descendants are not migrated automatically.

Checkpoints are immutable references. C1 and C2 remain independent even when
their Workspaces run in parallel. Rolling back C1 cannot move, delete, or
rewrite C2 or its Version. Rollback is Workspace-local by default and follows
the M3 rollback result contract: it preserves history, updates only the target
Workspace's physical/Snapshot and logical Version heads, and creates no new
Version. Resume from that result creates a new Execution; a later Snapshot/
Version is the new branch from the explicit base.

## Multi-Level Executions

Parent-child Execution ancestry, Handoff edges, Workspace ownership, and
rollback scope are independent relations. An E2 handoff does not reparent E2,
migrate E4/E5, or alter their Workspaces. Cycles remain forbidden by the M3
Execution Graph contract.

## Failure and Recovery

Provider failure, process/Agent crash, lease expiry, stale revision, Snapshot,
Restore, Rollback, Handoff, or Resume failure yields either the complete old
state, the complete new state, or a deterministic conflict/unknown result that
reopen can reconcile. A provider response is never phantom Core success.

Partial completion is valid: E1 may complete while E2 fails and E3 remains
interrupted. Reopen exposes each Execution independently and preserves an
interrupted/unknown outcome rather than marking it completed.

## Task Semantics

Task state is not the worst or last Execution state. A Task may remain
`running`/active while one Execution failed and another completed. Task
completion requires an explicit policy; this slice leaves aggregation and
completion policy open. Provider quota exhaustion is represented as an
Execution interruption/failure reason, without Core dependence on provider
names.

## Security and Compatibility

Parallel context may contain only opaque or redacted references. API keys,
OAuth tokens, passwords, cookies, private keys, raw secret prompts, and Agent
memory are forbidden. Different projects cannot share Workspaces, Versions,
Checkpoints, or Rollback scope by default. M1 v0.1.0, ADR-0016, all M2
head/Version/Snapshot/Operation meanings, and legacy readers remain unchanged.

## Schema Proposal

This is proposal only. Prefer the existing Workspace, Execution, Operation,
lease, and revision records. The existing Execution -> Workspace attachment
plus Workspace lease/revision can express the recommended model. Reconciliation
is transient/read-only under M2-SLICE-006; no durable reconciliation entity is
required by this proposal. Durable ownership transfer or a future explicit
collaboration mode would need a separately accepted relation and migration.
No table, column, DDL, or migration is added here.

## Open Decisions

Shared writable Workspace policy; Workspace ownership transfer; Version Head
selection arbitration; cross-Workspace/task coordination; selective or
cross-task rollback; reconciliation persistence; automatic conflict resolution;
Task status aggregation and completion policy; resource quota/fairness;
provider capability policy; scheduler/orchestration; Branch, Merge, and Rebase
(all **FUTURE**).

## Status

`PROPOSAL ONLY`. Runtime implementation belongs to M3-SLICE-003B after this
contract is accepted.
