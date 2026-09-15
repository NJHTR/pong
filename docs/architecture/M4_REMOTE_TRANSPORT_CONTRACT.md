# M4-012 Remote Agent Transport Contract

**Status:** `PASS / CONTRACT-FROZEN / REMOTE-DEPLOYMENT-NOT-PROVEN`

This contract defines how a remote Agent Runtime reaches the existing Pong
Core without creating a second domain protocol. It does not implement a network
server or credential verifier.

## One Protocol, Multiple Transports

```text
Local Runtime -> Local Transport -------------------------+
                                                          |
Remote Runtime -> Remote Transport -> Authentication      |
                                      -> Principal        |
                                      -> Session ---------+
                                                          v
External Agent Protocol v1.0 -> AgentControl -> Core -> Repository
```

JSONL, a future HTTP or WebSocket adapter, named pipes, Unix sockets, SDKs, and
an optional MCP adapter must all carry the same `ProtocolRequest`,
`ProtocolResponse`, Operation, Execution, ownership, lifecycle, error, retry,
and reconnect semantics. A transport must not introduce alternate Pong IDs,
transactions, or errors for domain outcomes.

## Identity and Lifecycle

| Concept | Meaning | Durability |
| --- | --- | --- |
| Connection | One transport channel | Ephemeral |
| Session | Authenticated interaction context | Ephemeral |
| Principal | Identity established by the security adapter | Ephemeral binding; credential state external |
| Agent | Provider-neutral Pong domain identity | Durable |
| Execution | One durable Agent attempt | Durable |
| Operation | Durable mutation/retry identity | Durable |

`Connection != Session`, `Session != Principal`, `Principal != Agent`, and
`Agent != Execution`. Connection loss never changes an Execution or Operation.

The remote lifecycle is:

```text
connect -> authenticate -> bind Principal -> discover access capabilities
        -> protocol hello -> request/response -> disconnect

reconnect -> re-authenticate -> bind a new session -> inspect/resolve durable
          state -> retry or continue
```

A heartbeat is not a domain requirement. A transport may use one for channel
liveness, but session expiry and request deadlines remain ephemeral and cannot
change durable Pong state.

## Authentication and Principal Handoff

The transport/security adapter validates credentials and produces:

```text
AuthenticatedPrincipal {
  principal_id,
  agent_ids
}
```

`RemoteAccessBoundary` accepts only this already-authenticated result. It does
not parse or store JWTs, OAuth tokens, API keys, TLS certificates, cookies, or
HTTP headers. A failed authentication creates no session.

Credential verification is `NOT_PROVEN`; the implemented handoff contract is
`PASS`. The adapter is responsible for proving that the supplied Principal and
Agent grants are authentic.

## Session and Authorization

Binding creates an expiring `RemoteSession` containing only opaque session and
Principal IDs plus host-supplied timestamps. Session IDs cannot be rebound.
Expiry, disconnect, invalidation, or Core restart removes the session only.

Before protocol dispatch, the boundary verifies that a non-`hello` request's
asserted `caller_agent_id` is granted to the Principal. The unchanged protocol
then enforces registered Agent and durable resource relationships:

```text
Principal grant -> Agent -> Task/Execution -> Workspace/Operation
```

Knowing an opaque Execution, Workspace, or Operation ID is insufficient.
Foreign Agent access returns `FORBIDDEN`; missing authenticated context returns
`AUTHENTICATION_REQUIRED`; expired context returns `SESSION_EXPIRED`.
Workspace writes still require lease, epoch, expiry, revision, and CAS.

## Capability Discovery

Remote access capabilities and Pong protocol capabilities are separate:

- `RemoteAccessBoundary::capabilities` describes authentication, rebind,
  session invalidation, disconnect, and reconciliation rules.
- protocol `hello` describes protocol version, commands, queries, mutation
  acknowledgement, provider neutrality, and transport neutrality.
- a concrete transport may separately advertise framing, streaming, message
  size, compression, or heartbeat support.

Transport features never become domain commands merely because an adapter
supports them.

## Retry and Uncertain Outcome

`request_id`, `operation_id`, `execution_id`, `agent_id`, `principal_id`, and
`session_id` remain distinct.

An exact retry of request `R1` and Operation `O1` returns the existing durable
Operation. A new request `R2` may create `O2`; Pong does not claim global
business-intent deduplication. If a response is lost, the client must reconnect
and query by request or Operation identity. It must not infer success or
failure from a socket timeout.

An absent Operation returns the existing protocol `NOT_FOUND` result. A
started, completed, failed, cancelled, or unknown Operation returns its durable
state. The contract does not add a second `UNKNOWN_OPERATION` error taxonomy.

## Restart and Cancellation

Client restart discards its connection/session but preserves its Principal-to-
Agent authorization source outside Pong. After re-authentication, a new session
can inspect the same Agent, Execution, Workspace, Checkpoint, Handoff, and
Operation.

Core restart invalidates all sessions. Repository state survives, and the
replacement Core performs existing cold-reopen recovery. Clients must
re-authenticate, rebind, inspect, and continue.

A transport timeout or connection close is not Operation cancellation. Pong
records cancellation only through an explicit protocol lifecycle mutation. It
does not claim to terminate an arbitrary remote process.

## Version Evolution

Protocol `1.0` remains frozen and exact-match. Unsupported versions receive
`UNSUPPORTED_VERSION`. The wire structs deny unknown fields, so adding fields
under the `1.0` label is not backward compatible and is forbidden.

Compatible capability growth may add advertised commands or queries that an
older client does not invoke. Any future wire-shape change or minor-version
negotiation requires a separate contract and tests. `v1.x` negotiation is
therefore `NOT_PROVEN`; this slice does not speculate about v2/v3.

## Error Layers

Remote access boundary errors:

- `AUTHENTICATION_REQUIRED`
- `AUTHENTICATION_FAILED`
- `SESSION_EXPIRED`
- `INVALID_PRINCIPAL`
- `INVALID_AGENT_BINDING`
- `FORBIDDEN`
- `CORE_UNAVAILABLE`

Domain/protocol errors remain the existing protocol error model, including
`UNSUPPORTED_VERSION`, `UNAUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`,
`REVISION_CONFLICT`, `LEASE_CONFLICT`, `INVALID_STATE`,
`IDEMPOTENCY_CONFLICT`, `RECOVERY_REQUIRED`, and `INTERNAL_ERROR`.

Transport framing, authentication, and domain errors must remain distinguishable
without exposing credentials, stack traces, SQL, or private filesystem paths.

## Observability

The ephemeral authorization context correlates `principal_id`, `session_id`,
`request_id`, asserted `agent_id`, and optional `operation_id`. Durable Pong
state correlates Operation with Execution, Workspace, timestamps, and outcome.
This answers mutation recovery questions without making every read or
authentication decision a durable domain event.

Durable authentication-decision/read auditing remains `PARTIAL / DEFERRED`.
Transport logs may add connection diagnostics but must never include secrets.

## Status Matrix

| Capability | Status |
| --- | --- |
| Remote connection model | PASS at transport-neutral contract level |
| Principal handoff | PASS |
| Remote authentication contract | PASS; credential verification NOT_PROVEN |
| Remote authorization | PASS at Principal/Agent and protocol ownership boundaries |
| Retry/idempotency | PASS |
| Uncertain outcome | PASS |
| Reconnect | PASS |
| Core restart | PASS in cold-reopen contract test |
| Client restart | PASS in session replacement contract test |
| Version negotiation | NOT_PROVEN; exact protocol 1.0 rejection is PASS |
| Error contract | PASS at boundary and protocol layers |
| Observability | PARTIAL |
| Real remote network transport | NOT_PROVEN |
| HTTP / WebSocket | DEFERRED |
| MCP | OPTIONAL ADAPTER / DEFERRED |
| Linux/macOS remote adapter parity | NOT_PROVEN |

## Validation

The transport-neutral contract suite passes with 18 active tests and no
ignored tests. The wider focused matrix passes with 53 active tests across the
remote boundary, External Agent Protocol, JSONL transport, lifecycle recovery,
Core ownership, local broker, Repository access policy, and security boundary.

Formatting, compilation, Clippy with warnings denied, and `git diff --check`
pass on Windows 11. The full regression remains `PARTIAL / ENVIRONMENT`: the
pre-existing `control_layer::three_provider_metadata_values_publish_to_isolated_workspaces`
diagnostic can fail during the combined suite when an independent
`Repository::open` receives Windows lock violation code 33. A focused rerun
passed, and no M4-012 code enters that unsupported direct multi-handle external
path. The event remains recorded rather than being ignored or reclassified as
a Remote Contract pass.
