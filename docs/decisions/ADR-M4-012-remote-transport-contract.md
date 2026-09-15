# ADR-M4-012: Remote Agent Transport Contract

- **Status:** `Accepted contract / Remote deployment not proven / Internal M4`
- **Date:** `2026-09-15`

## Context

Pong already has one frozen external Agent protocol, a single Core owner,
durable Operation recovery, and an architecture-level remote security boundary.
Before selecting HTTP, WebSocket, or MCP, it needs an executable contract for
authentication handoff, session lifetime, authorization, retry, and restart.

## Decision

Add a transport-neutral `RemoteAccessBoundary` in front of the unchanged
External Agent Protocol. A transport/security adapter authenticates credentials
and supplies an `AuthenticatedPrincipal` with explicit Agent grants. The
boundary creates an ephemeral expiring session and verifies the asserted Agent
binding before protocol dispatch.

The boundary owns no network listener, credential, durable session row,
Repository handle, or provider state. Core, protocol v1.0, AgentControl,
Operation identity, lease, revision, CAS, and durable error semantics remain
unchanged.

## Identity Decision

Connection, session, Principal, Agent, Execution, and Operation are separate.
Disconnect and timeout invalidate only ephemeral interaction state. Core restart
invalidates all sessions. Client and Core restart recovery require a new
authentication result and session followed by durable inspection or resolution.

## Capability and Version Decision

Remote access capabilities describe session/security behavior. Protocol
`hello` describes domain commands and queries. Concrete transports describe
framing or streaming independently.

Protocol `1.0` remains an exact, strict wire version. New fields cannot be added
under the same label because strict readers reject them. Future minor-version
negotiation is deferred and `NOT_PROVEN`; adding advertised commands/queries is
the only compatible capability growth allowed by this decision.

## Failure Decision

Transport timeout is an uncertain outcome, not cancellation. Exact request and
Operation retry uses the existing durable ledger. Connection close does not
alter Operation lifecycle. An explicit cancellation mutation may record intent
but does not claim remote process termination.

Remote access errors cover missing/failed authentication, invalid or expired
session/Principal binding, forbidden Agent grants, and Core unavailability.
All resource, revision, lease, state, idempotency, and durable recovery errors
remain protocol errors.

## Consequences

- Local and future remote clients enter the same External Agent Protocol.
- A future adapter can authenticate without teaching Core about JWT, TLS, HTTP,
  WebSocket, MCP, or provider vendors.
- Principal handoff and authorization can be tested without a network server.
- Production credential verification, TLS, rate limits, network backpressure,
  streaming, and cross-platform deployment remain `NOT_PROVEN`.
- HTTP, WebSocket, MCP, cloud sync, and Multi-Pong replication remain deferred.
