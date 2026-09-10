# M3 Parallel Reconciliation Contract

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003A  
**Authority:** M2-SLICE-006 point-in-time Workspace diff/reconciliation.

## Purpose and Boundary

Reconciliation determines whether an observation remains authoritative while
parallel Executions mutate Workspaces. It is an observation and classification
boundary, not a scheduler, merge engine, watcher, or conflict resolver. This
proposal adds no schema, DDL, background worker, provider, Branch, Merge, or
Rebase behavior.

## Observation Contract

An observation captures the authoritative identity tuple before scanning:
`workspace_id`, `project_id`, provider/driver, canonical locator binding,
Snapshot Head, Version Head where the operation requires it, environment
identity, and Workspace revision. The current tree or Snapshot is scanned using
the existing deterministic M2 rules. At completion, the durable Workspace row
and required Snapshot/manifest identity are read again.

If any authoritative field changes, the result is `UNSTABLE_OBSERVATION`
through the existing Conflict/error model. The scan must not return its old
diff as stable. A Workspace that changes and changes back is outside the
point-in-time guarantee and remains a known limitation of M2-SLICE-006; no
watcher or global lock is invented here.

## Parallel Cases

### Independent Workspaces

E1/W1 and E2/W2 may scan or mutate concurrently. Their revisions, Snapshot
Heads, Version Heads, leases, Operation identities, and results are disjoint.
The fact that they share Task or `base_version_id` is not a conflict.

### Same Workspace

A scan on W1 concurrent with a writer on W1 is an unstable observation unless
the scan's before/after authoritative tuple is unchanged. A second writer is a
Workspace/Lease/Revision Conflict under the parallel Workspace contract.

### Snapshot Publication

If Snapshot publication advances W1 head or revision during a scan, the scan
returns `UNSTABLE_OBSERVATION`. The newly published Snapshot remains durable
under the existing transaction contract; reconciliation reports the boundary
and does not roll it back.

### Restore or Rollback

Restore/rollback on W1 racing a W1 observation is treated like any mutation:
changed head, revision, environment, or identity is unstable. Equivalent work
on W2 is independent. Rollback never changes W2, its Version Head, or its
Checkpoint.

## Detection and Classification

The flow is:

1. Capture an immutable observation-start tuple.
2. Perform a read-only scan or provider-neutral read.
3. Re-read authoritative metadata and validate manifest/CAS integrity.
4. Compare all required fields, not only the content digest.
5. Return a stable result only when the tuple is unchanged and integrity
   checks pass; otherwise return `UNSTABLE_OBSERVATION`/Conflict.

Reconciliation never advances revision, acquires or renews a lease, changes a
Workspace or Version Head, creates an Operation, appends an event, or mutates
CAS.

## Conflict Ownership

The observer reports Reconciliation Conflict. The mutating Core path owns
lease/revision validation and transaction recovery. An Agent/controller may
choose a later retry or explicit policy, but automatic conflict resolution is
open and not implied. Provider metadata cannot downgrade an unstable result.

## Failure and Recovery

Missing/corrupt Snapshot, CAS, manifest, Workspace, environment, or scope
fails closed using existing integrity/conflict errors. Process or provider
failure yields old-or-new durable mutation semantics; an observation itself
cannot claim success when its authoritative tuple is unknown. Cold reopen reads
the durable Workspace and classifies any in-flight operation according to the
existing M2/M3 recovery rules.

## Version and Task Semantics

Parallel results based on the same Version are siblings in the immutable
Version Graph, not conflicts. A Version Head selection is a separate explicit
mutation and competing selections conflict deterministically. Task status is
not inferred from an observation or from the last parallel Execution.

## Security and Compatibility

Observation output contains no provider secrets, API keys, tokens, cookies,
private keys, or raw prompts. Opaque/redacted references are permitted. The
contract preserves M1 v0.1.0, ADR-0016, M2 Snapshot/Restore/Diff, Workspace
Head, Version identity/parent/Head, Operation identity, and legacy-reader
compatibility.

## Schema Proposal

No durable reconciliation record is required for the default point-in-time
model. Existing Workspace revision/head, lease, Snapshot/manifest, and
Operation boundaries are sufficient. If a future product policy needs a
durable observation audit or watcher, it must be a separate contract and
schema proposal; this slice does not reserve a table or column.

## Open Decisions

Watcher versus point-in-time only; durable observation audit; cross-Workspace
coordination; automatic retry/resolution; shared-writer collaboration mode;
Task completion aggregation; resource fairness; and future Merge/Branch/Rebase
(all **FUTURE**).
