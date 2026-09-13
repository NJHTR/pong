# ADR-M4-005: External Agent Protocol Architecture Freeze

- **Status:** `Accepted for internal M4 / TEST-GATED`
- **Date:** 2026-09-13
- **Decision type:** architecture boundary and transport evaluation

## Decision

Freeze Pong's provider-neutral External Agent Protocol as the formal external
control contract at protocol version `1.0`. Keep the layering:

```text
Transport or adapter -> External Agent Protocol -> AgentControl -> Pong Core
```

Pong Core owns durable execution/state/control semantics. AgentControl exposes
those semantics. The External Agent Protocol owns wire-safe DTOs, lifecycle,
identity, errors, capability discovery, and reconnect queries. Transport is
replaceable and must not define domain behavior.

JSONL over stdin/stdout remains the development/local transport. HTTP is the
leading future remote transport candidate, but no remote transport is selected
or implemented here. WebSocket, CLI, and SDK generation remain deferred.

MCP is explicitly an optional interoperability adapter:

```text
MCP -> Pong MCP Adapter -> External Agent Protocol -> AgentControl -> Core
```

MCP must never bypass the protocol or make LLM tool-call order the source of
Pong state correctness.

## Evidence And Boundaries

The decision is supported by active protocol, lifecycle/reconnect, transport,
Core, and AgentControl tests. Real Codex execution and prior local-native
handoff are separate evidence. Real Claude-over-JSONL remains
`BLOCKED / ENVIRONMENT` because the provider returned HTTP 403 quota exhausted.
Production authentication, remote deployment, live provider supervision, and
MCP implementation are `NOT_PROVEN`.

No schema migration, Core refactor, provider coupling, or transport-specific
domain API is introduced by this decision.

## Next Slice

Specify the authentication/authorization and observability requirements for a
remote transport before implementing HTTP or MCP. Do not start either merely
because it is available or popular.
