# ADR-M4-014: HTTP Production Hardening Boundary

- **Status:** `Accepted / Windows test-gated / Deployment capabilities partial`
- **Date:** `2026-09-19`

## Context

M4-013 carries unchanged External Agent Protocol v1.0 messages over HTTP. The
adapter was correct on Windows loopback, but production credential lifecycle,
rate limits, response limits, failure isolation, operational correlation, and
the exact slow-client boundary were not defined.

This decision hardens the existing adapter. It does not add a protocol, change
the Core schema, turn Pong into a network service, or implement TLS.

## Decision

Keep one `POST /v1/protocol` endpoint and the existing layering:

```text
TLS termination / production ingress
  -> HTTP adapter
  -> credential verifier
  -> authenticated Principal
  -> RemoteAccessBoundary authorization
  -> External Agent Protocol v1.0
  -> AgentControl
  -> single Core-owned Repository
```

The HTTP adapter adds these process-local controls:

- configurable request-body and response-size limits;
- a configurable fixed protocol-handler worker count;
- a per-principal fixed-window request limiter;
- handler panic containment;
- runtime counters and a bounded 64-entry request-correlation ring;
- atomic opaque-credential replacement and explicit revocation.

Limiter and observability state are intentionally not durable business state.
They reset when the process restarts. Transport metadata does not enter the
External Agent Protocol DTOs.

## Credential Decision

Bearer credentials remain opaque and provider-neutral. Only SHA-256 lookup
digests and Principal grants are held by the verifier. Credentials are not
written to Pong Core, durable evidence, correlations, or errors.

The development file loader rejects symlinks, non-files, sources over 1 MiB,
invalid JSON, empty grants, and duplicate credentials. On Unix it also rejects
group/other permission bits. Windows ACL validation and production secret
manager integration are `NOT_PROVEN` deployment responsibilities.

`reload-credentials` validates the complete replacement before an atomic swap.
New requests immediately use the new set. A request already admitted to the
dispatcher is allowed to finish. Rotation and revocation never cancel or
rewrite durable Executions or Operations.

## Network And TLS Decision

Plain HTTP is restricted to local development or explicitly trusted networks.
Production remote deployment requires TLS termination in a reverse proxy,
ingress, service mesh, or deployment platform. Pong does not own certificates,
private keys, or TLS handshakes.

Loopback remains the default. A wildcard, LAN, or other non-loopback bind
requires explicit `--allow-remote-bind` opt-in.

## Resource Boundary

Pong's protocol handler pool is fixed and configurable. This is not a global
connection bound. `tiny_http` 0.12 accepts connections into an internal
`TaskPool` that grows when busy and has an unbounded task queue; its public API
does not expose socket header/body read or idle deadlines.

Therefore these claims remain `NOT_PROVEN` inside Pong:

- global active-connection bound;
- bounded listener/parser queue;
- slow-header and slow-body protection;
- adapter-enforced end-to-end request deadline.

Production ingress must provide connection limits, a bounded pending queue,
header/body read timeouts, idle timeout, request deadline, and TLS. The
per-principal limiter protects admitted authenticated traffic but is not a
substitute for ingress controls.

## Failure And Error Decision

Each protocol handler is wrapped in a panic boundary. A panic fails the request
closed and increments a counter; it does not terminate the Core or prevent a
subsequent client request. Graceful shutdown stops accepting, drains bounded
protocol handlers, releases the Core owner, and does not synthesize Operation
success.

External errors remain stable and safe. They do not include filesystem paths,
SQLite details, Rust modules, stack traces, raw OS codes, Authorization headers,
or credential values. Internal diagnostic export is outside this adapter; the
programmatic metrics snapshot is the current observability boundary.

## Consequences

- External Agent Protocol v1.0 remains unchanged.
- Authentication remains in HTTP; authorization remains in
  `RemoteAccessBoundary`; domain concurrency remains in Core.
- Credential rotation and single-Core per-principal limiting are test-gated.
- Runtime metrics and bounded correlation are available without durable schema
  changes or secret exposure.
- Slow-client protection and a true global connection bound require production
  ingress or a future transport-library decision.
- TLS deployment, Internet deployment, Linux parity, macOS parity, and
  production credential operations remain `NOT_PROVEN`.
- MCP remains optional and deferred.

## Verification

`tests/http_hardening.rs` exercises rotation/revocation, atomic replacement,
rate limiting, metrics/correlation, response limits, panic isolation, safe
errors, resource-config rejection, explicit remote bind, and a real independent
`pong-agent-http` credential reload process (10 active tests). The M4-013 real
TCP/process suite continues to gate authentication, authorization, body limits,
concurrent clients, retry, reconnect, Core restart, graceful shutdown,
lease/revision conflicts, and JSONL equivalence (16 active tests).
The locked full regression remains `PARTIAL / ENVIRONMENT`: the unchanged
`agent_execution` process exited with Windows `STATUS_HEAP_CORRUPTION
(0xc0000374)` after its output; its previously localized performance test
passes in isolation.
