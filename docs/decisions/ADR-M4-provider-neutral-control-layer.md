# ADR-M4: Provider-Neutral Control Layer

- **Status:** `Proposed / Internal M3`
- **Date:** 2026-09-11
- **Scope:** local control facade used by the real Agent handoff validation
- **Decision type:** additive composition; no schema migration

## Context

Pong already persists Agent, Task, Execution, Workspace, lease, Snapshot,
Version, Checkpoint, Handoff, Resume, and rollback state. A real handoff test
needs a small stable call surface to compose those records while an external
Agent process edits a temporary Workspace. Calling SQLite and low-level
managers directly from every adapter would duplicate sequencing and make
provenance easy to omit.

## Decision

Add `AgentControl` as a thin Rust facade over the existing `Repository`,
`MetadataStore`, and `WorkspaceManager`. Requests are explicit typed structs;
responses are typed views. The facade does not own transactions or invent new
durable entities. Existing lower-layer validation remains authoritative.

## Invariants Preserved

The facade does not change M1 or M2 semantics. Version ownership, Version
identity, same-workspace Version parents, Workspace.head, Version Head,
Operation identity, Snapshot/CAS rules, lease ownership, and revision CAS stay
unchanged. Cross-workspace use passes an explicit source Version and produces
target-local materialized state.

## Provider-Neutral Boundary

`provider` is descriptive Agent metadata only. No branch in core behavior may
depend on Codex, Claude, Cursor, or another provider name. Provider processes
remain outside Pong and communicate through a future adapter boundary.

## Real-Agent Validation

The local E2E performs this sequence:

1. Register Codex and Claude Code identities and create one Task.
2. Create E1/W1, run a real `codex exec`, then persist a Version and C1.
3. Interrupt E1, create E2/W2 through Resume and a durable Handoff.
4. Materialize the source Version into W2 and run a real `claude -p`.
5. Persist W2's Version and C2, then reopen the repository and query all
   relations.

The test uses temporary workspaces and ephemeral/no-session-persistence
provider invocations. It verifies that continuation is discoverable from Pong
IDs rather than Git commits or hidden provider session state.

## Failure and Recovery

Provider exit failure is reported by the adapter/test and does not become a
durable Pong success. Pong writes continue to use existing transaction,
failpoint, lease, revision, materialization, and cold-reopen behavior. The
facade does not claim filesystem plus SQLite as one ACID transaction.

Version publication treats the request's observed Workspace revision as
authority rather than advisory metadata. The facade persists the original
Workspace revision, Workspace head, Version Head, Snapshot reference, and
Version Head update policy in the existing `version.create` Operation. A
retry reconstructs the same Operation envelope and relies on the existing
idempotency digest. It can finish an atomically committed Version after an
unconfirmed return, but refuses missing/foreign Snapshot state or any
unrelated Workspace revision/head transition.

`MetadataStore::create_version` remains the atomic boundary for the Version
row and completion of its Operation. Version Head remains a separate,
lease-guarded Workspace revision transition. An exact completed retry returns
the same Version and recognizes the already-completed Version Head update.

## Security

Requests contain opaque IDs, digests, and redacted context references only.
Credentials, tokens, raw prompts, transcripts, and provider memory remain
outside Core.

## Alternatives

- **Direct low-level calls in each adapter:** rejected because sequencing and
  provenance would be duplicated.
- **Provider-specific methods:** rejected because they couple Core to vendor
  behavior and make neutrality untestable.
- **MCP/SDK first:** deferred; transport and authentication are separate
  contracts and are not required to prove the local durable workflow.
- **New orchestration runtime:** rejected for this slice; scheduling,
  concurrency policy, and worker ownership remain outside the facade.

## Migration and Compatibility

No DDL or schema change is required. Existing repositories open unchanged;
legacy records retain their original meaning. The facade only calls existing
additive APIs and may be omitted by older readers.

## Open Decisions

Future work must define the external protocol and safe error payload,
adapter invocation contract, provider process timeouts and cancellation,
authentication ownership, remote execution, retry policy at the adapter
boundary, and any durable operation-to-Execution association. The protocol
transport remains deliberately undecided; MCP, CLI, HTTP, and SDK bindings are
not selected by this ADR.

## Consequences

The local handoff workflow is repeatable and its provenance is inspectable
without provider-specific storage. Conversely, this ADR intentionally stops
short of a production provider integration or a general agent orchestrator.
