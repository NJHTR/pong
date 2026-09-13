# ADR-M4: External Agent Protocol

- **Status:** `Proposed / Internal M4`
- **Date:** 2026-09-11
- **Decision type:** additive protocol facade; no schema migration

## Context

Pong Core and `AgentControl` already represent durable Agent, Task, Execution,
Workspace, Version, Snapshot, Checkpoint, Handoff, Resume, Operation, lease,
and revision state. External Agent runtimes need a stable contract without
depending on Rust records, SQLite, repository paths, or provider-specific
behavior.

MCP, HTTP, CLI, and local IPC solve transport and interoperability problems;
none should define Pong's domain contract. The contract must therefore exist
between transports and `AgentControl`.

## Decision

Add `protocol` as a transport-independent module above `AgentControl` with:

1. an explicitly versioned request/response envelope;
2. strict, explicit JSON DTOs separate from internal records;
3. tagged command/query operations;
4. safe machine-readable errors;
5. opaque Agent and domain identities;
6. explicit lease and revision authority for writes; and
7. reconnect queries over durable state.

Protocol version `1.0` is independent of Pong's repository generation,
migration, Operation schema, and provider version. JSON is the language-neutral
representation, not the transport.

## Provider Neutrality

Provider values are optional Agent metadata. Dispatcher behavior never branches
on Codex, Claude, Cursor, OpenHands, or any other runtime name. Agent identity,
Execution identity, and transport connection/session identity remain distinct.

## Workspace Binding

External requests carry an opaque `binding_ref`. A host-owned
`WorkspaceBindingResolver` resolves authorized references to paths. Wire
resources omit Workspace locator and driver fields, so a future remote client
does not need local filesystem access and cannot discover host paths from Pong
responses.

## Errors

Map Core failures to stable protocol codes and generic safe messages. Perform
explicit ownership, revision, lease, and lifecycle preflight checks where a
more precise code is required. Do not parse human-readable `PongError` text.
Only entity IDs and safe revision values may be returned as structured detail.

## Durability And Retry

A success is returned only after the existing synchronous Core operation
reports durable completion. The protocol does not introduce an accepted queue
or second transaction boundary.

Existing idempotency remains authoritative. Exact publication retries reach
`AgentControl` even after Workspace revision advancement so its Operation-based
recovery can return the durable result. Execution lifecycle repeats, lease
ensure/release, immutable resource creation, Checkpoint, Handoff, and Resume
reuse their existing replay semantics.

Materialization and lease renewal do not persist protocol request identities.
The client must inspect durable Workspace and lease state after an unconfirmed
response. Treating an arbitrary later state as a successful replay would hide
concurrent mutation and is rejected.

## Operation Ownership

Reuse the existing durable `execution_operations` table. Add thin
`AgentControl` methods to attach and list Operations. Protocol Version
publication attaches the `version.create` Operation to the owning Execution
after publication; an exact retry repairs a missing attachment idempotently.
No new entity or migration is required.

## Identity And Authorization

Version 1.0 registration and calls use asserted `caller_agent_id`. Unknown
identities are unauthorized, and Execution/Workspace writes are checked against
Execution ownership. This is an authorization-ready shape, not production
authentication. A transport must eventually authenticate and bind the asserted
ID before accepting untrusted remote requests.

## Cancellation

Expose `interrupt_execution` as a durable lifecycle transition. Do not claim
that Pong terminates an external process. A future runtime adapter may observe
the durable state and stop its process under a separate adapter contract.

## Alternatives

- **Serialize internal Rust records:** rejected because they expose unstable
  fields and couple compatibility to implementation details.
- **MCP-first or HTTP-first API:** rejected because transport would become the
  domain contract.
- **Provider-specific operations:** rejected because provider metadata is not
  semantic identity.
- **New idempotency or Operation schema:** rejected because existing durable
  identities and `execution_operations` already cover the implemented needs.
- **Global async acceptance queue:** deferred; it would require orchestration
  and durability semantics outside this milestone.

## Compatibility

The module is additive. Existing Rust API consumers, repository schemas, M1,
M2, M3, and `AgentControl` semantics remain unchanged. The only control facade
addition exposes an already-existing durable relation.

## Consequences

External clients can now express and inspect a provider-neutral handoff through
one stable in-process contract. The minimal `pong-agent-protocol` process maps
JSON Lines messages to this dispatcher without changing domain semantics. Its
single-component binding resolver constrains Workspace paths beneath a
host-owned root.

The local process transport is test-gated but does not prove an authenticated
remote deployment, provider SDK, MCP server, or process supervision. These
claims remain separate in tests and evidence.

## Next Slice

Run the existing real Codex and Claude Code workflow through the JSON Lines
transport when both authenticated CLIs are available. Keep `CORE`, `CONTROL`,
`PROTOCOL`, `TRANSPORT`, and `REAL_PROVIDER_E2E` evidence distinct.

## M4-004 Lifecycle Decision

The protocol reuses the existing Operation ledger and `execution_operations`
association. It does not add a request table, async queue, session entity, or
second transaction log. `request_id` resolves an uncertain response through
the caller/project scope; `operation_id` identifies the durable mutation;
`execution_id` identifies the Agent run; and `agent_id` remains durable Agent
identity. `start_operation` and `finish_operation` expose the existing
started/terminal lifecycle, with exact terminal replay and changed-outcome
rejection. `resolve_operation`, `get_operation`, and `inspect_execution`
provide cold-reopen recovery after process or connection loss.

An interrupted Execution and a started/unknown Operation remain observable and
require explicit reconciliation. Cancellation is a durable intent, not a
claim that an external process was killed. Transport session identity is never
used as Agent identity, and asserted identity remains an authorization-ready
boundary pending production authentication.
