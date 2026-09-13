# ADR-M4-006: Remote Control Security Boundary

- **Status:** `Accepted boundary / Internal M4`
- **Date:** 2026-09-13

## Decision

Remote transports authenticate an external caller and produce an ephemeral
Principal. The Principal is bound to a durable Pong `agent_id`, then passed to
the provider-neutral External Agent Protocol. Protocol authorization checks
resource ownership and delegates state authority to AgentControl/Core.

```text
Transport authentication -> Principal -> External Agent Protocol
    -> AgentControl -> Pong Core
```

Connections and sessions are never durable Agent or Execution identity.
Credentials and authentication context are transport-owned and are not stored
in Pong domain rows.

## Authorization

The minimal policy is ownership-based, not RBAC-heavy: Agent owns its
Executions; an Execution is bound to one Workspace; Operations are attached to
their Execution; Workspace writes require lease and expected revision. Reads,
control, and recovery operations use the same durable ownership and lifecycle
checks. Opaque IDs alone grant no access.

## Replay and Audit

Existing Operation identity, request resolution, lease, revision/CAS, and
recovery semantics remain authoritative. No second idempotency system is
introduced. Durable mutation evidence includes principal/Agent mapping,
request/Operation, Execution, Workspace, timestamps, lifecycle outcome, and
failure/retry data. Full authentication-decision and read auditing is deferred.

## Scope and Non-Goals

The local JSONL process is a trusted development boundary, not production
remote security. This ADR does not implement HTTP, HTTPS, JWT, MCP, a
distributed auth service, rate limiting, or a schema migration. Production
authentication is `NOT_PROVEN`; remote readiness is `PARTIAL` until those
transport-owned controls are specified and tested.
