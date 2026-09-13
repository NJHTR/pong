# M4-005 External Agent Protocol Architecture Freeze

**Status:** `PASS / INTERNAL / TEST-GATED`
**Checkpoint target:** `checkpoint: m4-protocol-architecture-freeze`
**Protocol version:** `1.0`

## Freeze Audit

| Area | Status | Evidence |
| --- | --- | --- |
| Pong Core domain state | Stable | Agent, Task, Execution, Workspace, Version, Snapshot, Checkpoint, Handoff, Resume, Rollback, Diff, Restore, Materialization, Operation, Lease, Revision are implemented in `src/` and covered by active integration suites. |
| AgentControl | Stable | `src/control.rs` is a provider-neutral facade over existing metadata/workspace APIs; lease, revision, CAS and recovery remain Core authority. |
| External protocol commands/queries | Stable | `src/protocol.rs`; `tests/external_agent_protocol.rs` and `tests/protocol_lifecycle_reconnect.rs`. |
| Operation ledger and Execution relation | Stable | Existing Operation rows plus `execution_operations`; lifecycle and cold-reopen tests pass. |
| Request/Operation/Execution/Agent identity | Stable | Separate fields and ownership checks; request resolution is scoped by project and Agent. |
| Lease, revision, CAS | Stable | Existing Core checks are preserved through the protocol; stale revision and foreign lease tests pass. |
| Reconnect and uncertain outcome | Stable | `resolve_operation`, `get_operation`, `inspect_execution`, fresh-process and cold-reopen tests pass. |
| Capability discovery/version | Stable for v1.0 | `hello` advertises protocol version and capabilities. Version negotiation is explicit; schema is strict. |
| Wire safety/errors | Stable | Explicit DTOs, safe error codes, no paths, SQL, stack traces, credentials, prompts, or transcripts. |
| JSONL transport | Stable development transport | `src/bin/pong-agent-protocol.rs`; process-boundary tests pass. It is not the domain contract or production trust boundary. |
| Authentication | Not yet externalized | Asserted Agent identity is authorization-ready; production credential binding is deferred. |
| Remote transports | Not yet externalized | HTTP, WebSocket, IPC and SDK bindings are architectural options only. |
| Live provider reconnect | Not proven | Protocol reconnect is proven; reconnecting a live external provider process is not. |
| Real Claude JSONL | Blocked / Environment | Provider returned HTTP 403 quota exhausted; no PASS claim is made. |

## Layer Boundary

```text
Agent Runtime
    -> optional interoperability adapter
    -> External Agent Protocol (frozen v1.0 contract)
    -> Transport adapter (JSONL now; others later)
    -> AgentControl
    -> Pong Core
```

Domain Protocol defines durable state and legal transitions. External Agent
Protocol serializes those control semantics. Transport carries the protocol.
MCP, if added, is an optional interoperability adapter and must call the
External Agent Protocol rather than bypassing it.

## Identity And Lifecycle

`request_id`, `operation_id`, `execution_id`, `agent_id`, `task_id`, and
`workspace_id` remain independent. A connection or process is not durable Agent
identity. `start_operation` creates a durable `started` Operation; terminal
outcomes are `completed`, `failed`, `cancelled`, or `unknown`. Exact retries
replay durable state, changed reuse is rejected, and `resolve_operation` finds
an uncertain request after a connection or process loss.

An Agent forgetting to call Pong is an integration failure, not a Core state
transition. A crash after a call leaves the durable Operation/Execution state
observable. Duplicate calls are governed by existing Operation identity and
idempotency. These black-box scenarios are covered at the protocol boundary;
provider/model compliance remains `NOT_PROVEN`.

## Transport Evaluation

Scores are 1 (poor) to 5 (strong) for Pong's control-plane requirements.

| Candidate | Lang | Local | Remote | Long-run | Stream | Reconnect | Retry | Auth | Concurrency | Observe | Ecosystem | Evolution | Ops | Recovery | Security | Portable | SDK | 3P | Control-plane fit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| JSONL stdin/stdout | 5 | 5 | 1 | 3 | 2 | 3 | 4 | 1 | 3 | 4 | 2 | 3 | 5 | 4 | 3 | 5 | 4 | 2 | 4 |
| CLI subprocess | 4 | 5 | 1 | 2 | 1 | 2 | 3 | 1 | 2 | 3 | 2 | 3 | 4 | 3 | 3 | 5 | 3 | 2 | 3 |
| HTTP/REST | 5 | 3 | 5 | 4 | 2 | 4 | 5 | 5 | 5 | 5 | 4 | 5 | 3 | 4 | 4 | 5 | 5 | 5 | 5 |
| WebSocket | 5 | 3 | 5 | 5 | 5 | 4 | 4 | 4 | 5 | 4 | 3 | 4 | 2 | 4 | 3 | 5 | 4 | 4 | 4 |
| MCP adapter | 4 | 4 | 4 | 3 | 3 | 2 | 2 | 3 | 3 | 2 | 5 | 3 | 3 | 2 | 3 | 4 | 3 | 5 | 3 |

The scores reflect current requirements, not popularity. JSONL is the best
development transport because it is deterministic, portable, easy to fault,
and already tested. HTTP is the strongest future remote transport candidate,
but is not selected or implemented in this slice. WebSocket is useful only if
streaming becomes a first-class requirement. CLI is a convenience wrapper,
not a superior control protocol. MCP scores high for tool interoperability but
low for durable lifecycle authority and reconnect semantics.

## MCP Decision

MCP is not Pong Core protocol, wire contract, or lifecycle authority. It is an
optional tool/ecosystem adapter. A future implementation must be:

```text
MCP -> Pong MCP Adapter -> External Agent Protocol -> AgentControl -> Core
```

It must not be `MCP -> Core`, and LLM tool-call order must not determine
Execution correctness. This preserves provider neutrality and permits adding
MCP without changing Core. The adapter is `NOT_PROVEN` because it is not built.

## Frozen Recommendation

1. Freeze External Agent Protocol v1.0 as the formal provider-neutral contract.
2. Keep JSONL as the development/local process transport.
3. Evaluate HTTP as the next remote transport only after authentication,
   authorization binding, observability, and deployment requirements are
   specified.
4. Keep WebSocket, CLI, and MCP deferred; MCP remains an adapter, never Core.
5. Generate future SDKs from the explicit protocol DTOs, not Rust records.

## Deferred Work And Risks

`NOT_PROVEN`: production authentication, remote authorization, HTTP/WebSocket,
MCP adapter, SDK generation, live provider process supervision, streaming
events, and distributed deployment. `BLOCKED / ENVIRONMENT`: real Claude JSONL
completion due provider quota. Strict v1 DTO decoding intentionally rejects
unknown fields; future compatible evolution must use explicit protocol version
negotiation or a separately specified compatibility rule.
