# M3 Cross-Workspace Immutable Source Model

**Status:** `PROPOSAL ONLY / Internal M3`  
**Slice:** M3-SLICE-003E

## Existing M2 Invariants

M2 keeps immutable Versions Workspace-owned. A Version references one immutable
Snapshot, and Snapshot metadata is also bound to the Workspace that published
it. The Snapshot root digest and its CAS manifest/content are immutable. A
Workspace `head` is the root digest of a Snapshot belonging to that same
Workspace. Restore, status, diff, and rollback validate these bindings before
reporting success. `Version.workspace_id`, Version identity, Version parentage,
Version Head, Operation identity, and M1 meanings remain unchanged.

## Current Conflict

An Execution in W2 can logically start from V100[W1], but directly assigning
S100[W1] to W2 would violate the existing Snapshot and Workspace Head checks.
The source and target therefore need separate representations.

## Alternative Models

### Model A: Workspace-owned Snapshot with Copy-on-Materialize

The source Version/Snapshot stays owned by W1. W2 reads the source manifest and
CAS objects, materializes them into its physical tree, then publishes a new
W2-local Snapshot metadata row. This adds a target Snapshot but does not copy
CAS bytes. It preserves all M2 ownership and Head invariants and is the safest
compatible model.

### Model B: Project-global Immutable Snapshot Source

This would make Snapshot metadata independent of Workspace. It conflicts with
M2 Snapshot ownership and requires a new content model. It is rejected here.

### Model C: Global CAS plus Workspace-local Snapshot Metadata

CAS is already content-addressed and reusable. Keeping source metadata local
while materializing a new target metadata row is the storage form of Model A.
It is compatible and requires no global Snapshot ownership change.

### Model D: Independent SourceReference Entity

A SourceReference relation could be useful if references acquire an independent
lifecycle or audit trail. The default Execution-level base reference does not
need that fifth durable entity. Adding one is deferred; if a future accepted
retry/recovery contract proves it mandatory, report `SCHEMA_GAP` before coding.

## Recommended Model

`MODEL_RECOMMENDATION = A`.

Model C is the compatible storage realization of this behavior: global CAS
content with Workspace-local Snapshot metadata. It is not a second ownership
model or a new durable entity.

Use an immutable source and a local result:

```text
V100[W1] -> S100[W1] -> CAS
                    |
                    +-- read-only materialize -> W2 tree -> S200[W2]
                                                         |
                                                         +-- W2.head
```

The recommendation preserves M2 without weakening any scope check. A
`base_reference_version_id` identifies the source only; it is not a target
Snapshot, Version parent, Version Head, or ownership transfer.

## Source

Source is an existing healthy immutable Version, its Snapshot metadata, and the
verified manifest/blob objects in CAS. Source validation must check existence,
project, environment, generation, migration, manifest integrity, and CAS
availability. Source reads never acquire the source Workspace lease or mutate
source metadata, heads, revisions, operations, or events.

## Target

Target is the explicitly authorized Workspace and its physical tree. A target
mutation requires the target Workspace lease and expected revision. The target
publishes target-local Snapshot metadata after materialization and verification.

## Snapshot Semantics

Source Snapshot and result Snapshot are different metadata records even when
their manifests have the same root digest. CAS objects may be shared. The
result Snapshot uses the existing local publication path and records the target
Workspace identity; it is not a duplicate Version and does not rewrite source
metadata.

## Workspace Head

`Workspace.head` continues to point only to the target Workspace's local
Snapshot root digest. After cross-Workspace restore/materialization, W2.head is
the digest of S200[W2], never S100[W1].

## Version Head

Version Head remains Workspace-local. A source Version may not be assigned to a
different Workspace's Version Head. A target may retain its existing local
selection or explicitly select a target-local Version under existing guards.

## Version Ownership and Parent

V100 remains owned by W1. Any Version later published for W2 is owned by W2 and
may use only a valid same-Workspace parent (or null). The source reference is
not a `parent_version_id` edge.

## Base Reference

The Execution-level base reference stores only the immutable source Version ID
(or an explicitly equivalent attachment). It grants read/materialize/diff
authority for the target Execution, never write authority over W1.

## Diff and Restore

Cross-Workspace diff compares the target physical tree with the verified source
manifest as a read-only observation. Cross-Workspace restore reads the source,
materializes the target, verifies it, and publishes a target-local Snapshot
using existing transaction/lease/revision and operation boundaries. Source and
target metadata remain independent.

## Rollback

The current M2/M3 rollback contract is Workspace-local and does not permit a
W2 record to target V100[W1]. A future cross-Workspace rollback can use this
source/target materialization model, but its local result Snapshot and
Version-Head semantics must be reconciled with the frozen rule that rollback
itself creates no Version. Until that contract is revised explicitly, such a
request is `CONTRACT_CONFLICT` and must fail closed.

## Parallel Workspace

W1, W2, and W3 can materialize the same source concurrently. Each has an
independent physical tree, local Snapshot metadata, head, Version Head, lease,
revision, operation, and recovery state. Shared CAS content is immutable.

## Failure Boundary and Recovery

Missing/corrupt source, missing CAS, scope mismatch, lease/revision conflict,
provider failure, or publication failure yields fail-closed old state,
new-complete state, or deterministic unknown/conflict. A source reference is
never reported as materialized until target verification and durable local
publication are complete. Reopen reconciles the existing operation and target
metadata; no source row is altered.

## Idempotency

The existing operation/request identity governs exact retries. A retry of an
already completed target materialization returns the same durable result after
verification. A changed source, target, or request identity is deterministic
conflict; no duplicate source mutation or phantom head is allowed.

## Migration and Legacy

No M2 ownership migration is proposed. A future additive Execution-level base
reference or materialization result linkage must leave existing rows untouched;
legacy v0.1 repositories receive no synthetic references. No DDL is executed in
this slice.

## Security

Only opaque Version/Snapshot IDs and bounded integrity metadata cross the source
boundary. Secrets, credentials, tokens, cookies, private keys, prompts, and
transcripts are never copied into target metadata.

## Open Decisions

Exact base-reference storage; target Snapshot publication linkage; whether
cross-Workspace restore is a distinct operation action; cross-Workspace
rollback result and Version Head policy; checkpoint reuse; and future
SourceReference lifecycle remain open.

## Status

`PROPOSAL ONLY`. This document does not implement source references,
cross-Workspace restore, or rollback.
