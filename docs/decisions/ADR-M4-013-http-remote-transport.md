# ADR-M4-013: HTTP Remote Transport

- **Status:** `Accepted implementation / Windows test-gated / Production security partial`
- **Date:** `2026-09-16`

## Context

M4-012 froze a transport-neutral Remote Agent contract. Pong now needs one real
remote transport without creating a second business API, changing External
Agent Protocol v1.0, or weakening the single Core owner policy.

## Decision

Add one synchronous HTTP endpoint, `POST /v1/protocol`, which carries unchanged
`ProtocolRequest` and `ProtocolResponse` values. Use `tiny_http` because Pong's
Core and protocol are synchronous and one endpoint does not justify converting
the Core to an async runtime.

Add `ProtocolDispatch` as the transport-facing Core interface.
`AgentProtocolCore` owns the one Repository and serializes dispatch. The HTTP
adapter depends only on this interface, credential verification, the
`RemoteAccessBoundary`, and protocol wire types.

The HTTP adapter uses a fixed worker pool. It authenticates each request with
an opaque bearer credential, obtains a Principal with explicit Agent grants,
creates an ephemeral per-request remote session, authorizes the asserted Agent,
and dispatches the unchanged protocol request.

The production development binary loads a strict external credential file,
acquires Core ownership before binding HTTP, defaults to `127.0.0.1`, and
requires explicit configuration for non-loopback binding.

## Error Decision

HTTP/access errors and protocol/domain errors remain separate. HTTP status is
adapter metadata. Protocol error codes such as `REVISION_CONFLICT`,
`LEASE_CONFLICT`, `FORBIDDEN`, and `UNSUPPORTED_VERSION` remain in the unchanged
`ProtocolResponse` body.

## Lifecycle Decision

HTTP connection and keep-alive state are never Pong identity. Every request
re-authenticates. Disconnect and timeout do not cancel durable Operations.
Reconnect resolves state with existing request, Operation, Execution, Agent,
and Workspace IDs. Core restart invalidates ephemeral network state but keeps
Repository state.

Graceful shutdown stops accepting work, drains in-flight workers, and then
releases the Core owner. A second Core is rejected before it can listen.

## Security Decision

No anonymous mode or default non-loopback exposure is provided. Body size and
worker count are bounded, CORS is absent, and diagnostic responses are safe.

The adapter intentionally does not implement TLS, OAuth, OIDC, JWT, certificate
management, rate limiting, or credential rotation. Plain HTTP is limited to
loopback/trusted development use. Production deployment requires external TLS
termination and ingress controls. Configurable slow-client/read timeout remains
`NOT_PROVEN` in this stack.

## Consequences

- HTTP and JSONL use the same External Agent Protocol v1.0 semantics.
- Core, AgentControl, Repository, and domain models do not know HTTP.
- Multiple HTTP clients share one Core owner and existing lease/revision/CAS.
- Static authentication is real but intentionally minimal and external to Pong
  durable state.
- The new dependency is a small synchronous HTTP implementation; Tokio, Hyper,
  Axum, Actix, WebSocket, and MCP are not introduced.
- Production network security and Linux/macOS parity remain `NOT_PROVEN`.
