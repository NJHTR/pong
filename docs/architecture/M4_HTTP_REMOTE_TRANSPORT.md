# M4-013 HTTP Remote Transport / M4-014 Production Hardening

**Status:** `M4-013 IMPLEMENTED / M4-014 HARDENED ADAPTER / WINDOWS TEST-GATED / PRODUCTION DEPLOYMENT PARTIAL`

M4-013 adds HTTP as the first remote carrier of the frozen External Agent
Protocol v1.0. It does not add an HTTP-shaped domain API.

## Architecture

```text
HTTP client
  -> POST /v1/protocol
  -> opaque credential verification
  -> Authenticated Principal
  -> per-request RemoteAccessBoundary session
  -> ProtocolDispatch
  -> AgentProtocolCore
  -> External Agent Protocol v1.0
  -> AgentControl
  -> single Core-owned Repository
```

`src/http_transport.rs` has no Repository, SQLite, WorkspaceManager, provider,
or MCP dependency. The HTTP binary acquires Core ownership once, constructs an
`AgentProtocolCore`, and passes only its `ProtocolDispatch` interface to the
transport. Concurrent HTTP workers never open independent Repository handles.

## Endpoint

The only domain endpoint is:

```text
POST /v1/protocol
Content-Type: application/json
Authorization: Bearer <opaque credential>
```

The request body is an unchanged `ProtocolRequest`. The response body is an
unchanged `ProtocolResponse`. HTTP does not generate request IDs, Operation
IDs, Execution IDs, revisions, leases, or alternate domain errors.

Other paths and methods are transport errors. CORS is not enabled. Responses
use `application/json`, `Cache-Control: no-store`, and
`X-Content-Type-Options: nosniff`.

## Authentication

The development binary reads a strict JSON credential source supplied with
`--credentials-file`. The source is external configuration, not Pong durable
state. Each opaque credential maps to a Principal and explicit Agent grants;
the credential is not an Agent ID. In-memory lookup keys are SHA-256 digests so
the verifier does not retain the raw lookup string as its map key.

HTTP requests authenticate independently. A successful verification creates a
short-lived `RemoteAccessBoundary` session for that request, authorizes the
asserted Agent, and destroys the session before returning. HTTP keep-alive is
not a Pong Session. Reconnect and client restart therefore re-authenticate and
reconstruct authorization without changing durable Agent, Execution, or
Operation identity.

This is a minimal provider-neutral credential mechanism. `StaticCredentialVerifier`
indexes SHA-256 digests in memory and never persists or logs bearer values. The
development binary accepts only a regular, non-symlink credential file no larger
than 1 MiB. Unix group/other permission bits must be clear; Windows ACL
verification is not implemented and remains a deployment prerequisite.

Credential replacement is an atomic in-memory operation. The control command
`reload-credentials` fully parses and validates the replacement before swapping
it in. A rotation can add B, then remove A, without cancelling or rewriting any
durable Execution or Operation. HTTP requests authenticate independently, so new
requests observe the replacement immediately; an already dispatched request is
not forcibly cancelled. Production secret-manager distribution and Windows ACL
parity remain `PARTIAL / NOT_PROVEN`.

## Network Security Defaults

The default listen address is `127.0.0.1:8743`; library tests use an ephemeral
loopback port. Binding a non-loopback address requires the explicit
`--allow-remote-bind` flag. Anonymous mode is not available. The default body
limit is 1 MiB and the server uses eight fixed workers, avoiding an unbounded
thread per request.

The adapter is plain HTTP. It is suitable for loopback and explicitly trusted
development networks. Production remote deployment requires external TLS
termination and policy controls. Pong does not implement cryptography, OAuth,
OIDC, JWT, or certificate handling in Core.

The adapter has configurable request-body and response-size limits, a fixed
protocol-handler worker count, and an in-memory per-principal fixed-window rate
limiter. Limiter state is intentionally ephemeral and resets with Core restart.
The selected `tiny_http` 0.12 listener creates its own connection parser
`TaskPool`: it starts four threads, grows when all are busy, and has an internal
unbounded task queue that Pong cannot configure. It also exposes no safe
adapter-level socket header/body read deadline. Therefore global connection
concurrency, queue bounding, and slow-client/read-deadline protection are
`NOT_PROVEN`; production ingress must enforce connection caps, bounded queues,
header/body read timeouts, idle timeouts, and request deadlines before traffic
reaches Pong.

## Error Layers

Transport/access errors use HTTP status plus a wire-safe access body:

| HTTP | Meaning |
| --- | --- |
| 400 | malformed transport/access input |
| 401 | missing, invalid, or expired credential |
| 403 | Principal is not granted the asserted Agent |
| 404 | HTTP endpoint does not exist |
| 405 | HTTP method is not supported |
| 413 | request body exceeds the configured limit |
| 415 | body is not JSON |
| 503 | Core dispatcher is unavailable |

Protocol failures keep their original `ProtocolResponse` body. The adapter
maps them to an HTTP status without changing the domain error code. For
example, `REVISION_CONFLICT` and `LEASE_CONFLICT` use HTTP 409 while the Core
continues to know only the protocol error.

No response exposes credential values, Repository paths, SQL, stack traces, or
Rust/OS error details.

## Retry, Timeout, and Recovery

HTTP retry reuses the caller-supplied `request_id` and `operation_id`. Exact
retry returns the existing durable Operation; HTTP never synthesizes a second
Operation. If a client sends the complete request and disconnects before the
response, a new connection can resolve the request identity and inspect the
durable outcome.

Connection close and client timeout do not cancel an Operation. Cancellation
remains an explicit External Agent Protocol lifecycle mutation.

Core restart destroys HTTP connections and ephemeral sessions. Repository
state survives. A replacement HTTP Core reacquires `core-owner.lock`, accepts a
fresh authenticated request, and can inspect the prior Operation or Execution.
A simultaneous second HTTP Core fails before listening.

## Concurrency

Eight HTTP workers accept independent clients. `AgentProtocolCore` owns one
Repository behind a mutex, so protocol mutations are serialized through the
single authority. This permits concurrent clients and isolated Executions and
Workspaces without introducing additional storage owners.

Workspace concurrency remains unchanged:

- reads follow existing protocol visibility;
- writes require an Execution binding and Workspace lease;
- lease epoch and expiry reject the wrong writer;
- revision/CAS rejects stale state;
- HTTP adds no second concurrency system.

## Server Lifecycle

The production adapter starts in this order:

1. parse external configuration and credential source;
2. validate the Workspace binding root;
3. acquire the Repository Core owner;
4. start the loopback HTTP listener;
5. emit a JSON readiness line;
6. accept requests.

`shutdown` on stdin, or stdin EOF, stops workers, drains currently executing
requests, closes the listener, drops the Core, and releases Repository
ownership. Process termination relies on the OS owner-lock release and normal
Repository cold-reopen recovery.

The adapter catches panics at the protocol-handler boundary. A panicking
request is failed closed and counted; subsequent requests continue through the
remaining fixed workers. This does not change the `tiny_http` connection
parser's independent resource behavior.

Development invocation:

```text
pong-agent-http --repository PATH --workspace-root PATH \
  --credentials-file PATH [--listen IP:PORT] [--allow-remote-bind]
```

The credential file is a strict object containing a `credentials` array. Each
entry has `credential`, `principal_id`, `agent_ids`, and optional
`expires_at_ms` fields. Credential files are deployment secrets and must not be
placed in Pong evidence or Repository state.

## Protocol Equivalence

Contract tests send identical protocol envelopes through real JSONL and real
TCP HTTP transports. Success responses and unsupported-version errors are byte
equivalent as parsed JSON. The full HTTP lifecycle test covers Agent A work,
Version publication, Checkpoint, Handoff, Agent B Resume, materialization,
continued work, and a second Checkpoint.

## Status Matrix

| Capability | Status |
| --- | --- |
| HTTP transport | PASS on Windows loopback |
| HTTP to Protocol equivalence | PASS |
| Static credential authentication | PASS / production identity and Windows ACL PARTIAL |
| Principal/Agent authorization | PASS |
| Credential rotation/revocation | PASS at adapter contract; production secret operations PARTIAL |
| Retry and uncertain outcome | PASS |
| Reconnect | PASS |
| Core restart | PASS |
| Multi-client functional behavior | PASS |
| Same-Workspace lease/revision protection | PASS |
| Security defaults | PASS for local development / production PARTIAL |
| Per-principal in-memory rate limit | PASS for one Core; state resets on restart |
| Global connection bound / queue bound | NOT_PROVEN (`tiny_http` internal pool) |
| Slow-client/read deadline | NOT_PROVEN / external ingress requirement |
| Response-size limit | PASS at adapter boundary |
| Handler panic isolation | PASS at adapter boundary |
| Request correlation / metrics | PASS / PARTIAL (runtime snapshot; no durable or secret-bearing logs) |
| TLS / HTTPS | NOT_PROVEN / external termination required |
| Provider neutrality | PASS |
| Offline Core | PASS; HTTP remains optional |
| Linux/macOS HTTP parity | NOT_PROVEN |
| MCP | OPTIONAL ADAPTER / DEFERRED |

## Validation

The M4-014 hardening suite passes 10 active tests and the retained M4-013 HTTP
suite passes 16 active tests, with no failures or ignored tests. `cargo fmt`,
locked compilation, Clippy with warnings denied, and `git diff --check` pass.

The M4-014 focused suites pass. A complete `cargo test --all --locked` rerun
reached the unchanged `agent_execution` suite, then its test process exited
with Windows `STATUS_HEAP_CORRUPTION (0xc0000374)`. The previously localized
performance test passes when run unchanged in isolation; no M4-014 HTTP code
participates in that test. This aggregate-process observation is retained as
`PARTIAL / ENVIRONMENT` and is not hidden, ignored, serialized, or weakened.
