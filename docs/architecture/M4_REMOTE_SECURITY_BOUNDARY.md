# M4-006 Remote Control Security Boundary

**Status:** `PASS / INTERNAL / ARCHITECTURE-TEST-GATED`

This document defines the security boundary required before a remote
transport is added. It does not claim production authentication.

## Layering

```text
Transport security / authentication
    -> authenticated Principal
    -> External Agent Protocol authorization
    -> AgentControl
    -> Pong Core
```

Core and AgentControl do not inspect HTTP headers, JWTs, MCP metadata,
WebSocket state, or filesystem tokens. A transport adapter authenticates a
caller and supplies a principal binding to the protocol. The protocol checks
that binding against durable Agent, Task, Execution, Workspace, and Operation
relationships. Core remains the authority for state transitions, leases,
revisions, CAS, and recovery.

## Audit Results

| Boundary | Status | Finding |
| --- | --- | --- |
| Core security boundary | PASS | Core is provider-, transport-, and MCP-neutral. |
| Authentication model | NOT_PROVEN | v1.0 accepts asserted `caller_agent_id`; no production credential verification exists. |
| Authorization model | PASS / INTERNAL | Ownership and lifecycle checks deny unknown or foreign Agents; remote principal binding is deferred. |
| Resource ownership | PASS | Agent -> Execution -> Workspace and Operation -> Execution relations are enforced; knowing an ID is insufficient. |
| Replay/idempotency | PASS | Existing Operation identity, request resolution, revision/CAS, and lease semantics remain authoritative. |
| Session/reconnect model | PASS / INTERNAL | Connection and session are ephemeral; durable Agent/Execution/Operation state survives process and transport restart. |
| Observability | PARTIAL | Durable Operations answer mutation outcome; authentication decisions and every read are not a durable audit stream. |
| Remote readiness | PARTIAL | DTOs use opaque IDs and safe errors, but authentication, rate limits, TLS, and deployment policy are not implemented. |

## Principal Model

An authenticated transport produces an ephemeral Principal:

```text
Principal {
  agent_id: opaque durable Agent identity,
  session_id: ephemeral connection/session identity,
  authentication_context: transport-owned, never persisted by Pong
}
```

`session_id != agent_id`, `connection != session`, and neither is an
Execution. Agent identity survives process restart, connection restart, and
transport replacement. Credentials, tokens, keys, and authentication context
must not be stored in Pong domain rows.

The current local JSONL adapter is a trusted local-process boundary. It passes
the asserted Agent identity directly to the protocol and is not a production
remote trust boundary. A future remote adapter must authenticate first, then
bind the authenticated Principal to the asserted Agent or reject the request.

## Resource Ownership

```text
Principal -> Agent -> Task -> Execution
                                  \-> Workspace
                                  \-> Operation
```

- Agent may inspect and mutate only its own Execution lifecycle.
- An Execution may operate only on its explicitly bound Workspace.
- Workspace writes require the current lease, valid epoch/expiry, and expected
  revision; stale callers receive `REVISION_CONFLICT`.
- An Operation is readable/resolvable only in the caller Agent/project scope
  and is attached to its owning Execution through `execution_operations`.
- Checkpoint, Handoff, Resume, Restore, Rollback, and Version operations retain
  existing Core ownership and lease/revision checks.
- Possessing an opaque resource ID does not grant authority.

## Minimal Authorization Matrix

| Operation | Resource scope | Class | Required authority |
| --- | --- | --- | --- |
| inspect / get / diff | Agent, Task, Execution, Workspace, Version, Snapshot, Checkpoint, Handoff, Operation | read | authenticated Agent scope; source references remain read-only |
| create Task / Execution / Workspace | project and caller Agent | write | authenticated Agent allowed for project; existing Core validation |
| start / finish Execution or Operation | caller-owned Execution and its Workspace | control | Execution ownership; running state; lease where required |
| checkpoint / handoff / resume | related Task/Execution/Workspace | control/recovery | owning Agent plus existing lifecycle, lease, and revision checks |
| restore / rollback / materialize | target Workspace and immutable source | recovery | target Execution ownership, lease, revision, source validation |
| acquire / renew / release lease | Workspace | control | Execution-to-Workspace binding and lease authority |

The matrix intentionally has no roles. Future policy can add project-level
authorization at the Principal -> Agent boundary without changing Core
semantics.

## Replay, Idempotency, and Uncertain Outcomes

`request_id`, `operation_id`, `execution_id`, and `agent_id` have separate
responsibilities. A repeated identical request may replay the existing durable
Operation result. A changed request reusing an Operation identity is rejected.
After a transport timeout, the client resolves by Operation or request identity;
it must not assume success from connection state. Lease renewal and operations
without durable request rows require state inspection after uncertainty.

No second idempotency table is allowed. No transport may turn a retry into
last-writer-wins behavior.

## Session and Connection Semantics

Ephemeral: socket, PID, connection ID, session ID, timeout timer, and runtime
buffer. Durable: Agent, Task, Execution, Workspace, Version/Snapshot,
Checkpoint, Handoff/Resume, Operation, lease epoch/expiry, revision, and
recovery state. A reconnect creates a new session and reuses durable opaque
identities.

## Observability and Audit Boundary

The minimum answerable mutation question is:

> Which authenticated principal, mapped to which Agent, at what time, issued
> which request/Operation for which Execution and Workspace, and what durable
> outcome resulted?

Durable domain evidence: Operation identity, request identity, Agent,
Execution, Workspace, timestamps, lifecycle status, failure code, lease/revision
transitions, and recovery records. Runtime-only diagnostics may contain
connection timing and transport faults but must not contain tokens, credentials,
prompts, transcripts, private paths, SQL, or stack traces.

The current ledger provides durable mutation observability. A complete durable
authentication-decision/read audit stream is deferred and therefore this area
is `PARTIAL`.

## Local vs Remote Assumptions

Local JSONL assumes a trusted host process and constrained local binding root.
Remote transport must additionally provide authenticated principals, replay
protection, request size/time limits, rate limiting, TLS or equivalent channel
security, credential rotation, connection timeout policy, and audit export.
HTTP itself supplies none of these automatically.

## Deferred Implementation

No HTTP/HTTPS server, JWT, MCP server, distributed auth service, database
migration, or large refactor is part of M4-006. The next implementation slice
must specify the principal-binding and observability interfaces before choosing
a remote transport.
