# ADR-M4-010: Local Core Broker and Ephemeral Runtime Sessions

- **Status:** `Accepted contract / Partial implementation / Internal M4`
- **Date:** 2026-09-15

## Context

M4-009 established one long-lived Core owner as the recommended Repository
authority. The remaining question is how multiple local Agent Runtimes relate
to that owner without turning Core into a network service or making connection
state durable domain state.

Protocol v1.0 is frozen. Its durable request, Operation, Execution, Agent, and
Workspace identities already survive reconnect. The current JSONL transport is
one attached stdin/stdout stream and is not a multi-client daemon.

## Decision

Define Runtime Session as ephemeral transport context managed outside the
durable Pong schema. A session supports connect, local trust/authentication
boundary, capability discovery, request/response, disconnect, invalidation,
and reconnect through a new session.

The Core remains the only Repository state authority. Runtime loss never
deletes durable state. Core loss invalidates sessions but does not change
durable outcome. Replacement Core recovery and Runtime reconciliation use the
existing protocol and Operation ledger.

Do not repurpose durable `Operation.session_id` as a transport connection ID.
Changing it on reconnect would change the durable Operation envelope and break
exact retry. No Runtime-session database table or protocol v1.0 field is added.

## Current Transport Decision

Keep JSONL stdio as a single attached development/local transport. Do not turn
it into a daemon during this slice. Use a test-only local multiplexing harness
to validate one real Core process with multiple real Runtime processes and
ephemeral sessions. This is Core validation, not a production transport claim.

## Consequences

- Core, Runtime, session, Agent, Execution, and Operation identities remain
  distinct.
- Same Agent across multiple Runtimes and Executions is allowed.
- Same Execution concurrent reads and distinct Operations are allowed;
  lifecycle mutations remain revision-controlled.
- Same Workspace writes remain lease/epoch/revision/CAS-controlled.
- Graceful synchronous shutdown drains the active request, rejects later
  harness requests, preserves unresolved Operations, and releases ownership.
- Production multi-client local transport and ephemeral session observability
  require a later adapter slice.
- External Runtime rollback remains `NOT_PROVEN` because rollback is not a
  frozen protocol v1.0 command.
- HTTP, MCP, authentication services, remote sync, replication, and provider
  coupling are not introduced.
