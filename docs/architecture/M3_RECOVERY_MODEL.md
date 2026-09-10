# M3 Recovery Model for Handoff / Checkpoint / Rollback / Resume

**Status:** `IMPLEMENTED / INTERNAL / TEST-GATED`
**Slice:** M3-SLICE-002D - Rollback Result / Materialization Core
**Implementation:** Durable Handoff, Checkpoint, Rollback-record, and Resume
paths are implemented in MetadataStore with SQLite transaction boundaries and
real integration tests. M3-SLICE-002C closed the rollback result contract;
M3-SLICE-002D adds local Workspace materialization, dual-head publication,
prepared-intent retry, and reopen recovery. Provider/workspace adapters other
than the local driver remain out of scope.

## Purpose

This model defines what a cold reopen may report when a multi-Agent control
operation crosses a process, provider, filesystem, or SQLite commit boundary.
The central invariant is old-or-new durable visibility. An uncertain caller
result is never promoted to success from an in-memory observation.

## Durable Authorities

Recovery uses the existing authorities rather than inventing parallel state:

- Task and Execution rows describe durable coordination state.
- Version and Snapshot rows describe immutable logical/content state.
- `Workspace.head` remains the Snapshot root digest.
- `Workspace.version_head_id` remains an independent Version selection.
- Workspace leases and revision CAS authorize writes.
- The existing Operation ledger records initiating intent/outcome.
- Future Handoff, Checkpoint, Rollback, and Resume records add relation
  identity and scope; they do not replace these authorities.

## State Machines

Relation status is not Task or Execution status. The proposal uses:

```text
Handoff:    requested -> accepted -> completed
                         |          |
                         +-> failed +-> unknown
                         +-> cancelled

Checkpoint: absent -> complete

Rollback:   requested -> applying -> completed
                         |            |
                         +-> failed   +-> unknown
                         +-> cancelled

Resume:     requested -> created -> running/paused/terminal Execution
                         |
                         +-> unknown
```

The full status vocabulary remains a future contract decision. The current
core persists complete relation rows with deterministic terminal metadata;
rollback materialization uncertainty is not hidden as a successful restore.

## Handoff Recovery

### Before commit

If validation, authorization, lease, or persistence fails before commit:

- no Handoff relation is visible;
- source and target Execution rows remain at their prior durable states;
- no target becomes an active owner;
- retry with the same request is allowed.

### After commit, before acknowledgement

Cold reopen must find either the complete prior state or the complete Handoff
state, including source/target references, actor, request digest, and status.
It must never infer completion from the target's provider response. Exact retry
returns the same Handoff; a changed request conflicts.

### Lease boundary

Handoff does not transfer a Workspace lease. A target may only write after a
valid lease acquisition. If the source and target use one Workspace, lease
serialization is an independent prerequisite and must be visible in the
existing lease/revision records.

## Checkpoint Recovery

Checkpoint creation is a reference transaction over an already durable healthy
Version. It does not publish new Version bytes. A crash yields either no
Checkpoint or one complete immutable Checkpoint. A row pointing to a missing,
corrupt, wrong-generation, or wrong-scope Version is an integrity error, not a
recoverable success.

Exact retry recognizes the same canonical request and returns the original
Checkpoint. A changed Version, scope, actor, or reason under the same request
identity is deterministic key reuse.

## Rollback Recovery

Rollback has two distinct phases:

1. durable intent and target validation;
2. provider/workspace restoration and durable result publication.

Before restoration commit, no current Workspace/Version selection changes. If
restoration has an uncertain external effect, the relation is `unknown` and
reopen/reconciliation must inspect the actual durable Workspace state before
publishing a result. A partial tree, dangling `Workspace.head`, or direct
Version parent rewrite is forbidden.

The successful result restores the target Snapshot into the authorized
Workspace, sets `Workspace.head` to that Snapshot digest, and sets
`Workspace.version_head_id` to the target Version through the existing
lease/revision-guarded selector. Rollback creates no Version and leaves
`result_version_id` null; a later Resume is the Version-producing operation.
Existing target and historical Versions stay immutable.

Execution-scoped rollback is isolated to one target Workspace by default. A
Task-scoped rollback must enumerate every affected Execution/Workspace and is
not implied by an Execution rollback.

## Resume Recovery

Resume creates a new Execution attempt from an explicit Version or Checkpoint.
The old Execution is never reopened or rewritten. Before commit, no new
Execution is visible. After an uncertain commit, cold reopen finds either no
new attempt or one complete attempt linked to its source. Exact retry returns
the same new Execution; changed source or actor semantics conflict.

## Failure Classification

| Condition | Durable classification | Allowed next step |
| --- | --- | --- |
| Validation/authorization failure | no mutation / deterministic error | correct request |
| Provider failure before durable effect | failed relation, no phantom success | retry only if contract marks retryable |
| Pre-commit SQLite failure | old state | exact retry |
| Post-commit acknowledgement loss | unknown to caller, complete row on reopen | inspect/retry deterministically |
| Missing/corrupt reference | integrity failure | repair through explicit future contract; never infer |
| Stale lease/revision | conflict | reacquire/re-read, never overwrite |

## Idempotency and Concurrency

All four control actions use a canonical request identity/digest and the
existing Operation identity where an initiating action exists. Concurrent
identical requests converge to one durable record/result. Concurrent requests
with different targets, scope, source, or actor fail deterministically under
the existing conflict model. No operation is considered successful merely
because a provider callback returned success.

## Parallel and Nested Recovery

Parent Execution edges and Handoff edges are independent. Recovery of E1 does
not reparent E2, delete E3, or roll back sibling Workspaces. A nested handoff
preserves the complete parent chain. Task-level coordination is explicit and
auditable; a child cannot silently acquire project-wide authority.

## Compatibility and Legacy

The proposal is additive and has no effect on v0.1 source meaning. Existing
readers need not understand future relation tables. No synthetic legacy
Checkpoint, Handoff, Rollback, Resume, or Task baseline is generated during
migration. M1 event/projection semantics, M2 Version/Head semantics, and
Operation identity remain unchanged.

## Security Boundary

Recovery records retain opaque/redacted references only. Provider credentials,
prompt transcripts, OAuth tokens, cookies, passwords, private keys, and raw
adapter memory are excluded. An opaque reference is not authorization.

## Contract Coverage

The ignored contract file covers Handoff H1-H10, Checkpoint C1-C8, Rollback
R1-R10, Resume S1-S6, and multi-Agent interaction M1-M8. It is a design index,
not runtime evidence. The M3-SLICE-002C result contract remains indexed by
`tests/rollback_result_contract.rs` (RR1-RR22), while
`tests/rollback_result.rs` provides 30 real runtime cases for the implemented
local materialization path.

## Status

`M3-SLICE-002D = PASS / INTERNAL / TEST-GATED`.

The local implementation prepares a rollback intent, materializes and verifies
the target Snapshot in a temporary sibling tree, atomically replaces the local
Workspace tree, and publishes both heads plus the completed result under the
existing lease/revision guard. Metadata pre-commit and post-commit failpoints,
materialization failure, missing/corrupt CAS, exact retry, cold reopen, and
legacy readability are covered by runtime evidence. No DDL, provider
integration, release tag, or push was performed.
