# ADR-M4-016: Remote Operability & Failure Semantics

- **Status:** Accepted / Windows process-gated / deployment partial
- **Date:** 2026-09-20

## Decision

Keep External Agent Protocol v1.0 and Core as the authorities for domain
semantics. Add only transport-local, bounded diagnostics and a bounded
process-local rate-limiter principal set. Preserve the distinction between
transport failure, Protocol failure, durable Operation outcome, and Core
availability. A disconnected client recovers by querying durable state; HTTP
must not manufacture cancellation or duplicate work.

Diagnostics are a fixed 64-entry ring with stable outcome categories and
identity fields. Runtime metrics remain in-memory and are not durable business
state. Shutdown emits a best-effort final local snapshot but always prioritizes
draining workers and releasing Core ownership.

No Protocol DTO, Core schema, MCP adapter, rollback operation, cancellation
protocol, distributed service, or provider-specific behavior is introduced.

## Consequences

Failed Operations remain Protocol results and are not mechanically mapped to
HTTP 500. 401/403/409/429/503 retain transport meanings. An unbounded stream of
unique principals cannot grow the limiter without bound; capacity exhaustion
fails closed with 429. `tiny_http` parser-pool/slow-client limits, TLS,
production secret management, Linux/macOS, and public Internet deployment
remain `NOT_PROVEN` and require deployment controls.
