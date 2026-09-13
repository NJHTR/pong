# ADR-M4-007: Offline-First Pong Core

- **Status:** `Accepted architectural constraint / Internal M4`
- **Date:** 2026-09-13

## Decision

Pong Core is offline-first, local-first, self-contained, durable, and network-
independent. Agent execution and state control must work using local SQLite,
CAS, filesystem workspaces, and the Rust API without HTTP, WebSocket, MCP,
cloud services, remote databases, or external authentication.

```text
Local interface -> External Agent Protocol -> AgentControl -> Pong Core
Remote interface -> security/principal -> External Agent Protocol
                 -> AgentControl -> Pong Core
```

Transport, authentication, and interoperability adapters are optional layers.
MCP is an optional adapter and may only call the External Agent Protocol.

## Evidence

The real `offline_core_e2e` integration test executes the complete durable
Agent workflow and verifies state after cold reopen using temporary local
directories. Dependency and runtime inspection found no network or provider
startup requirement. Existing Core, protocol, and transport tests remain
unchanged and no schema migration is required.

## Consequences

Remote failure cannot prevent local Core startup or invalidate local durable
state. Future sync or remote transport must preserve existing Operation,
lease, revision, CAS, and recovery semantics. Production authentication,
remote deployment security, and MCP implementation are not implied by this
ADR and remain `NOT_PROVEN` or deferred.
