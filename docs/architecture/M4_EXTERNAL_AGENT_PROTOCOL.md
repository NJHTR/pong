# M4 External Agent Protocol

**Status:** `IMPLEMENTED / INTERNAL / TEST-GATED`

M4-004 lifecycle/reconnect semantics are included in this internal slice.

**Protocol version:** `1.0`

**Scope:** provider-neutral, transport-independent control contract

## Boundary

The implemented layering is:

```text
Agent Runtime
    -> future Transport
    -> External Agent Protocol
    -> AgentControl
    -> Pong Core
```

The protocol defines explicit serializable DTOs and dispatch semantics. It is
not a network service, process manager, provider adapter, authentication
system, or second persistence layer. JSON is the version 1.0 representation.
The first local process transport uses JSON Lines; HTTP, MCP, and SDK bindings
remain optional transport choices above this contract.

## Envelope

A request carries:

- `protocol_version`: external protocol version, independent of repository
  schema and provider versions.
- `request_id`: stable caller request identity used by operations that have a
  durable request-id contract.
- `caller_agent_id`: asserted opaque Agent identity when the operation is not
  `hello`.
- `issued_at`: stable caller timestamp used by existing durable identity and
  idempotency rules. It is not lease authority.
- `operation_id`: required only where Pong's existing durable Operation model
  owns the mutation, currently Version publication.
- `operation` and `payload`: a tagged command or query with a strict schema.

The host supplies `now_ms` directly to the dispatcher. A client cannot extend
or revive a lease by forging `issued_at`.

A response echoes protocol, request, and Operation identities and contains
exactly one of a typed result or a safe protocol error. A mutating success
means the underlying core returned durable completion. The protocol does not
return an asynchronous "accepted" state.

## Commands

Version 1.0 implements:

- Agent registration using an asserted identity.
- Task, Workspace, and Execution creation.
- Execution start, pause, completion, failure, and interruption.
- Workspace lease acquisition, renewal, and release.
- Version publication and explicit Execution current-Version selection.
- Checkpoint, Handoff, and checkpoint Resume creation.
- Cross-workspace immutable Version materialization.

Interruption is a durable Execution state transition only. Pong does not claim
to kill an external provider process.

Rollback and destination-only restore are not exposed in version 1.0. The
cross-workspace continuation workflow uses `materialize_version`, which
publishes a target-local Snapshot and Workspace Head through the existing M3
contract. Adding rollback requires a dedicated external DTO for its complete
lease, revision, checkpoint, and result semantics; it must not be inferred
from materialization.

## Queries

Version 1.0 implements hello/capability discovery; lookup for Agent, Task,
Execution, Workspace, Version, Checkpoint, Handoff, and Operation; Execution
inspection; Task checkpoint and Handoff lists; and read-only Workspace diff
against an immutable Version.

`inspect_execution` returns the Execution's Agent, Task, optional Workspace
and active lease, base/current Versions, direct Checkpoints, Handoffs where the
Execution is source or target, Resume record, and attached Operations. It is a
read model reconstructed from durable rows and remains available after a cold
reopen.

## Wire Resources

Protocol resources are explicit DTOs. Pong's internal Rust records are not
serialized directly. Opaque domain identifiers remain stable, while internal
fields are intentionally omitted, including:

- Workspace driver and filesystem locator.
- repository, SQLite, lock, and process paths.
- Operation input/output payloads, policy details, and internal error text.
- generation, migration, request digest, and redaction implementation fields.
- credentials, prompts, transcripts, provider sessions, and secrets.

A Workspace is created through an opaque `binding_ref`. The host implements
`WorkspaceBindingResolver` and returns an authorized local path to Core; the
resolved path never appears in a protocol response.

## Identity And Authorization

Agent identity is a durable opaque ID. Provider and display-name values are
optional metadata and never select behavior. Connections, PIDs, executable
names, and provider sessions are not Agent identity.

Version 1.0 has no production authentication. `caller_agent_id` is an asserted
identity. Unknown callers receive `UNAUTHORIZED`; an Agent attempting to
mutate another Agent's Execution or acquire its Workspace lease receives
`FORBIDDEN`. A future authenticated transport can bind credentials to the same
Agent/Task/Execution/Workspace authorization inputs without changing the
domain protocol.

## Concurrency And Leases

Execution and Workspace mutations retain Core's lease and revision checks.
Acquiring a lease requires an Execution owned by the caller and bound to the
target Workspace. Repeating acquisition while the same Agent holds the active
lease returns that authority; a foreign holder yields `LEASE_CONFLICT`.

Stale expected revisions yield `REVISION_CONFLICT` with safe expected and
actual values. Exact Version publication and current-Version retries are
recognized before stale revision rejection so Core's durable replay rules can
finish or return the original result. A stale caller cannot overwrite newer
Workspace or Execution state.

## Idempotency And Recovery

Version 1.0 reuses existing Core identities and idempotency:

- Resource creation is replayed by explicit immutable resource identity.
- Execution lifecycle repeats return the already-reached identical state.
- Lease acquisition is an idempotent ensure operation for the same holder;
  release recognizes the same durable epoch tombstone.
- Version publication uses `request_id` plus `operation_id` and Core's durable
  Operation replay. The Operation is attached to its owning Execution through
  the existing `execution_operations` relation.
- Checkpoint, Handoff, and Resume use their existing durable identities and
  request digests.
- Queries and diff are read-only and repeatable against unchanged state.

Lease renewal and materialization do not have a durable protocol request row.
After an unconfirmed response, a client must inspect the Workspace/lease before
issuing a new request. A stale materialization retry fails with
`REVISION_CONFLICT`; the protocol does not manufacture a replay result.

Reconnect does not depend on a prior transport session. Clients recover using
`get_operation`, `get_workspace`, `get_execution`, `inspect_execution`, and
checkpoint/Handoff discovery.

### Operation lifecycle and reconnect

External runtimes keep `request_id`, `operation_id`, `execution_id`, and
`agent_id` distinct. `start_operation` durably creates a `started` Operation
attached to a running, caller-owned Execution. `finish_operation` records one
terminal state: `completed`, `failed`, `cancelled`, or `unknown`. Terminal
retries with identical content replay the durable record; changed content is
rejected. `resolve_operation` finds an Operation by project, caller Agent, and
request identity after a lost response. `get_operation` exposes its owning
Execution while enforcing Agent ownership.

The existing Operation ledger remains authoritative. A started or unknown
Operation is not interpreted as successful completion; clients must inspect it
and choose recovery, resume, or handoff using the existing Execution state.
Connection and process restarts do not change Agent identity or durable state.
Cancellation records intent or interruption; Pong does not claim to terminate
an external process. `hello` advertises the lifecycle commands and queries
without coupling the protocol version to transport or provider versions.

## Error Contract

Errors use stable machine codes:

`UNSUPPORTED_VERSION`, `VALIDATION_ERROR`, `UNAUTHORIZED`, `FORBIDDEN`,
`NOT_FOUND`, `CONFLICT`, `INTEGRITY_ERROR`, `LEASE_CONFLICT`,
`REVISION_CONFLICT`, `INVALID_STATE`, `IDEMPOTENCY_CONFLICT`,
`NOT_SUPPORTED`, `RECOVERY_REQUIRED`, `RESOURCE_EXHAUSTED`, and
`INTERNAL_ERROR`.

Safe details may contain entity kind/ID and expected/actual revision. Raw
`PongError` messages, stack traces, SQL, filesystem paths, and OS diagnostics
do not cross the boundary. Retryability is explicit but advisory; clients must
still inspect durable state after an uncertain result.

## Operation Association

No schema addition was required. `execution_operations` already provides a
durable one-Operation-to-one-Execution ownership relation with project, Agent,
and Workspace validation. `AgentControl` now exposes thin attach/list methods,
and protocol Version publication attaches the completed `version.create`
Operation to its Execution. Execution inspection returns the safe Operation
summary.

## Validation

`tests/external_agent_protocol.rs` exercises version negotiation, strict JSON,
safe error responses, asserted identity and cross-Agent denial, lease/revision/
lifecycle errors, exact retries, locator suppression, Operation ownership,
cross-workspace diff, the complete Runtime A to Runtime B handoff flow, and
cold-reopen discovery. Runtime names in this suite are metadata only.

## Local JSON Lines Transport

`pong-agent-protocol` is the minimal process-boundary adapter. It opens one
existing repository, reads one JSON request per stdin line, supplies host time,
dispatches through `ExternalAgentProtocol`, and writes one JSON response per
stdout line. Empty lines are ignored and malformed envelopes receive a safe
`VALIDATION_ERROR`; stdout contains no diagnostic prose.

The adapter accepts `--repository` and `--workspace-root`. A binding reference
is one ASCII alphanumeric, dot, underscore, or hyphen component beneath the
canonical binding root. Absolute paths, separators, `.` and `..` are rejected
before Core sees a Workspace creation request. Core retains its own filesystem
and repository-boundary validation.

`tests/external_agent_protocol_transport.rs` starts the compiled binary as a
child process and sends raw JSON through pipes. It covers malformed input,
path traversal rejection, the complete two-Runtime Checkpoint/Handoff/Resume
and cross-workspace materialization workflow, process shutdown, a fresh
transport process, and durable inspection without session memory.

## Not Proven

Production authentication/authorization, network transport/security,
reconnecting a live provider process, process cancellation, HTTP, MCP, SDKs,
remote execution, provider-specific adapters, scheduling, and distributed
workers are `NOT_PROVEN`. The local JSON Lines transport is development-only;
it is not a production trust boundary.
