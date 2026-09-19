# ADR-M4-015: Real Remote E2E Boundary

- **Status:** `Accepted / Windows process-gated / Deployment capabilities partial`
- **Date:** `2026-09-20`

## Context

M4-013 and M4-014 established a correct, hardened HTTP adapter, including real
TCP tests. Most tests still hosted either the server or client logic in the
test process. Pong therefore needed executable evidence that independent OS
processes could traverse the complete HTTP, security, protocol, Core ownership,
and Repository path without redefining any layer.

## Decision

Gate the supported topology with independent `pong-agent-http` and
`pong-agent-http-client` processes over an OS-selected loopback TCP port. The
test process owns only temporary fixtures and bounded child supervision. Every
domain request crosses HTTP parsing, credential verification,
`RemoteAccessBoundary`, Protocol v1.0 dispatch, `AgentControl`, the single Core,
and the real SQLite/CAS Repository.

The validation client remains binary-private and is not a public SDK. The
server gains only a local stdin metrics snapshot command and two additional
runtime metrics (`worker_threads` and `peak_active_requests`) needed to observe
the existing fixed handler pool. Transport metadata remains outside protocol
DTOs and durable Core state.

Preserve these existing authorities:

- HTTP verifies opaque credentials;
- `RemoteAccessBoundary` binds Principal to Agent;
- External Agent Protocol v1.0 carries domain intent;
- Core owns Operation idempotency, revision, lease, lifecycle, and recovery;
- one Core process exclusively owns one Repository.

Credential reload affects subsequent per-request sessions immediately. It does
not cancel an admitted request or mutate a running durable Execution or
Operation.

## Test Isolation Decision

Each case creates separate temporary Repository, Workspace root, and credential
directories, then binds `127.0.0.1:0`. Readiness and control messages use the
server's local stdin/stdout channel, while every protocol interaction uses an
independent client process and real TCP HTTP. Drop guards close stdin, wait for
normal exit, and use bounded termination only as cleanup fallback.

## Compatibility Decision

Protocol v1.0 and the Core schema remain unchanged. HTTP and JSONL process
scenarios compare protocol outcomes for success, failure, retry, revision and
lease conflicts, missing Operations, and reconnect. HTTP status remains
transport metadata.

External Agent Protocol v1.0 has no rollback operation. Rollback is therefore
recorded as `NOT_PROVEN / PROTOCOL CAPABILITY GAP`; this milestone does not
break the frozen contract merely to expose an internal Core capability.

## Consequences

- Real independent-process Windows loopback E2E is proven.
- Abrupt and graceful Core lifecycle, second-Core exclusion, reconnect,
  durable continuation, failure isolation, and adapter resource limits are
  executable gates.
- The existing private HTTP adapter remains optional; Pong Core remains
  offline-first and provider-neutral.
- `tiny_http` global connection/parser queue bounds and slow-client deadlines
  remain outside the proven adapter boundary.
- Plain HTTP remains restricted to local development or explicitly trusted
  networks. Production requires external TLS termination and ingress limits.
- TLS deployment, public Internet operation, production secret management,
  Windows credential ACL enforcement, Linux/macOS parity, and production
  observability export remain `NOT_PROVEN`.
- MCP remains optional and deferred.

## Verification

`tests/real_remote_e2e.rs` contains eight active independent-process tests.
The focused HTTP, protocol, Core ownership, broker, and hardening suites remain
green. The full regression retains the pre-existing intermittent Windows raw
OS code 33 result in the unsupported direct-handle concurrency diagnostic; the
unchanged failing test passes when rerun alone.
