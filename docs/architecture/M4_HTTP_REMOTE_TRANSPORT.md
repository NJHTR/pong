# M4-013 HTTP Remote Transport

**Status:** `IMPLEMENTED / WINDOWS TEST-GATED / PRODUCTION DEPLOYMENT PARTIAL`

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

This is a minimal static credential mechanism. Credential-file permissions,
distribution, rotation, revocation, rate limiting, and production identity
integration remain deployment concerns and are `NOT_PROVEN`.

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

The selected synchronous HTTP library does not expose an adapter-level
configurable slow-client/request-read deadline. Client deadlines and operation
cancellation remain separate; hardened production ingress timeouts and
backpressure are `NOT_PROVEN`.

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
| Static credential authentication | PASS / deployment identity NOT_PROVEN |
| Principal/Agent authorization | PASS |
| Retry and uncertain outcome | PASS |
| Reconnect | PASS |
| Core restart | PASS |
| Multi-client functional behavior | PASS |
| Same-Workspace lease/revision protection | PASS |
| Security defaults | PASS for local development / production PARTIAL |
| TLS, rate limiting, hardened ingress timeout | NOT_PROVEN / deployment boundary |
| Provider neutrality | PASS |
| Offline Core | PASS; HTTP remains optional |
| Linux/macOS HTTP parity | NOT_PROVEN |
| MCP | OPTIONAL ADAPTER / DEFERRED |

## Validation

The dedicated HTTP suite passes 16 active tests with no failures or ignored
tests. A focused Core/transport/security matrix passes 70 active tests with no
failures or ignored tests. `cargo fmt`, locked compilation, Clippy with warnings
denied, and `git diff --check` pass.

The first full regression attempt reached the retained M4-008 direct
multi-process diagnostic and observed Windows raw OS error 5 after successful
cold-reopen validation. An unchanged focused rerun passed while still observing
defined code 2/code 33 fail-closed outcomes. The subsequent complete
`cargo test --all --locked` run passed. No test was ignored, serialized, or
weakened to obtain the final result.
