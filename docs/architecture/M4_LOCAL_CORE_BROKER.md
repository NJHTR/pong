# M4-010 Local Core Broker and Runtime Sessions

**Status:** `PASS / INTERNAL / POLICY-FROZEN`

Pong's normal external-Runtime model is one local Core owner coordinating
multiple Runtime sessions. The Core is a local process coordinator, not an
HTTP, MCP, remote, or distributed service.

## Identity Boundaries

- **Core process:** the one live owner of a Repository and its state authority.
- **Runtime:** a client process executing Agent behavior.
- **Runtime session:** ephemeral connection context between a Runtime and Core.
- **Agent:** durable provider-neutral identity asserted through the protocol.
- **Execution:** durable identity for one Agent attempt on a Task.
- **Operation:** durable mutation/work identity with retry semantics.
- **Connection:** transport-specific channel carrying one session.

These identities are not interchangeable. One Core may serve many Runtimes;
one Runtime may act for an Agent; one Agent may own many Executions; multiple
Runtime sessions may access the same Agent or Execution subject to existing
authorization, lease, and revision rules.

## Repository Authority

The Core owns `Repository`, `MetadataStore`/SQLite, CAS, Workspace filesystem
mutation, Operation lifecycle, and startup recovery. External Runtimes do not
open those stores. They submit opaque IDs and structured protocol requests.

The Rust Repository API remains supported for Core implementation, embedded or
internal use, tests, and offline maintenance. It is Model A and is not the
default external Runtime concurrency entrance. Its known Windows direct-handle
startup limitation remains `PARTIAL`.

## Runtime Session Lifecycle

The minimum session contract is:

```text
CONNECT
  -> local trust/authentication boundary
  -> capability discovery
  -> request/response
  -> DISCONNECT or INVALIDATE

RECONNECT
  -> new ephemeral session
  -> query durable IDs
  -> resolve Operation outcome
  -> continue
```

Disconnect or invalidation removes only connection context. It does not delete
or roll back Agent, Task, Execution, Operation, Workspace, Version, Snapshot,
Checkpoint, Handoff, Resume, or Rollback state.

Runtime sessions are intentionally not added to the durable database. The
frozen protocol v1.0 wire envelope is unchanged. In particular, durable
`Operation.session_id` remains part of the existing stable Operation envelope;
it is not replaced with an ephemeral connection identifier because doing so
would break exact retries after reconnect.

## Local Trust Boundary

The current JSONL development transport relies on local process trust and the
protocol's registered Agent checks. It does not add JWT, OAuth, credentials, or
provider-specific authentication. A future remote transport must authenticate
a Principal and authorize it before entering the unchanged protocol and
AgentControl boundary.

## Core Lifecycle

```text
startup
  -> acquire single-owner access
  -> open and validate Repository
  -> recover unfinished durable state
  -> accept local protocol requests
  -> stopping
  -> stop accepting new work
  -> finish the synchronous in-flight request
  -> retain started/unknown Operations as such
  -> close transport and Repository
  -> release ownership
```

Current JSONL handling is synchronous: one decoded request is durably completed
or returns an error before the next line is read. EOF is graceful shutdown. It
does not convert a previously started Operation into success.

Forced Core termination invalidates all attached connections. The OS releases
the owner fence, durable state remains, and a replacement Core performs cold
reopen and recovery. Runtime reconnect uses a new session and the same durable
request, Operation, Execution, Agent, and Workspace identities.

## Concurrent Runtime Semantics

| Scenario | Contract |
| --- | --- |
| Different Agents and Executions | Allow; state remains isolated. |
| Same Agent, different Executions | Allow; Agent identity does not collapse Execution identity. |
| Same Execution, concurrent reads | Allow after Agent authorization. |
| Same Execution, distinct Operations | Allow while Execution state permits; Operation IDs remain distinct. |
| Same Execution lifecycle transition from stale revision | One transition wins; stale caller receives `REVISION_CONFLICT`. |
| Same Workspace writers | Agent/Execution/Workspace authorization plus lease, epoch, expiry, revision, and CAS apply. |
| Exact Operation retry after reconnect | Return the existing durable Operation. |
| Runtime crash after Operation start | Core stays alive; Operation remains durable and resolvable. |

No Runtime-level global lock is introduced.

## Transport Boundary

`pong-agent-protocol` remains a single attached stdin/stdout stream. One
process instance does not provide simultaneous independent client connections.
It may carry requests for multiple Agent identities, but it is not a
multi-client daemon.

M4-010 uses a test-only filesystem multiplexing harness with one real Core
process and multiple real Runtime child processes. The harness owns ephemeral
connect/disconnect/invalidation context and feeds unchanged protocol v1.0
requests into one `ExternalAgentProtocol`/Repository owner. It validates Core
coordination but is not a production transport or a claim that JSONL stdio is
multi-client.

Named pipes, Unix sockets, localhost HTTP, SDK, and MCP remain possible future
adapters. None defines Core semantics.

## Verified Process Scenarios

- one Core with three concurrently running Runtime client processes;
- three distinct Agents, Tasks, Executions, and completed Operations;
- two Runtime processes using the same Agent with isolated Executions;
- two Runtime processes inspecting and starting Operations on one Execution;
- one successful and one stale conflicting lifecycle transition;
- Runtime process exit after submitting an Operation without reading response;
- another session resolving that durable Operation while Core remains alive;
- explicit session invalidation and rejection of later requests;
- graceful stopping drains queued work and rejects a new connection;
- a started Operation remains started after graceful shutdown;
- forced Core termination, replacement ownership, cold reopen, and inspection;
- second Core and direct Repository access rejection from M4-009;
- checkpoint, handoff, resume, materialization, and reconnect through the
  production JSONL process E2E.

Cross-workspace rollback remains covered through Core/AgentControl tests but is
not exposed as a protocol v1.0 command. Rollback through an external Runtime
session is therefore `NOT_PROVEN / PROTOCOL CAPABILITY GAP`; this slice does not
change the frozen protocol to conceal that boundary.

## Status

| Capability | Status |
| --- | --- |
| Repository ownership | PASS on Windows focused validation |
| Core lifecycle | PASS on Windows focused validation |
| Multi-Runtime via one Core | PASS in real-process harness |
| Production JSONL multi-client transport | NOT_PROVEN / single attached stream |
| Runtime session contract | PARTIAL; ephemeral harness implemented, production identity implicit |
| Runtime crash isolation | PASS in real-process harness |
| Core crash recovery | PASS on Windows |
| Second Core rejection | PASS on Windows |
| Graceful shutdown | PASS for synchronous broker/JSONL semantics |
| Same Agent / multiple Runtimes | PASS in real-process harness |
| Same Execution / multiple Runtimes | PASS in real-process harness |
| Durable session observability | NOT_APPLICABLE; session is ephemeral |
| Ephemeral core/runtime/session observability | PARTIAL / harness only |
| Direct Repository API | SUPPORTED INTERNAL / MODEL A; not default external access |
| Windows direct multi-process handles | PARTIAL / IMPLEMENTATION LIMITATION |
| Linux/macOS | NOT_PROVEN |
| Remote sync | DEFERRED |
| MCP | OPTIONAL ADAPTER / DEFERRED |

## Validation Result

The focused M4-009/M4-010 aggregate passed `135` tests with no failures or
ignored tests. This includes the real-process broker, Core ownership, protocol,
transport, reconnect, concurrency, handoff, checkpoint, and rollback targets.

The initial required `cargo test --all --locked` run was not green. The active
M4-008 Model A diagnostic
`real_multi_process_workspace_publications_are_atomic_or_fail_closed` observed
one direct Repository Workspace writer fail closed with Windows raw OS error
`5` (`PermissionDeniedWithOsError`). Other writers made durable progress, and
the failure did not occur through the one-Core broker path. Code `5` was not an
accepted result in that diagnostic, so the failure remains retained rather than
reclassified or hidden.

M4-011 separated the release gate from unsupported direct external access,
added a deterministic real-process Core-owner bypass test, and improved the
existing diagnostic to verify cold-reopen Workspace heads before reporting an
unclassified failure. The final full regression passed. M4-010 is therefore
ready under the frozen Core-owned external Runtime policy; production
multi-client transport remains a separate capability.
