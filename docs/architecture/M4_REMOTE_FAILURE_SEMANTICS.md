# M4-016 Remote Operability & Failure Semantics

**Status:** `PASS / WINDOWS PROCESS-GATED / DEPLOYMENT PARTIAL`

M4-016 freezes the distinction between transport, protocol, Operation, and Core
failure without changing External Agent Protocol v1.0, Core schema, or domain
authorization. HTTP remains an optional adapter over the same single Core.

## Audit conclusions

1. The correlation chain is caller `request_id`, optional Protocol
   `operation_id`, authenticated `principal_id`, asserted `agent_id`, and any
   available `execution_id`/`workspace_id`.
2. `request_id` is caller-created, copied into Protocol responses and durable
   Operation records, and ends as a bounded transport diagnostic entry. It is
   not an HTTP-specific Protocol field.
3. `operation_id` is traceable for commands that require one and is retained in
   response, durable Operation, correlation, and diagnostics.
4. `execution_id` is traceable when carried by the Protocol payload or returned
   Operation. Commands without Execution scope correctly have no such value.
5. Principal and Agent are correlated after authentication/authorization;
   Workspace is correlated when present in payload or lease authority.
6. Request failure and Operation failure are distinct. A durable failed
   Operation is a Protocol success result and remains HTTP 200.
7. TCP disconnect does not cancel or fail an Operation. The client observes an
   unknown outcome until durable reconciliation.
8. Exact retry cannot create a duplicate Operation because idempotency remains
   Core/Protocol-owned.
9. Unknown outcome recovery uses `get_operation`/`resolve_operation` with the
   durable identity before retry. M4-015 proves the lost-response window using
   an independent client process.
10. Reconnect obtains latest revision by an explicit read; stale mutation is
    rejected before refreshed retry.
11. Core restart invalidates ephemeral sessions but preserves durable
    Execution, Operation, Version, Checkpoint, Handoff, and Resume state.
12. Credential revoke rejects subsequent per-request authentication; it does
    not rewrite already admitted or durable work.
13. Credential rotation atomically replaces grants. HTTP has no long-lived
    authenticated session, so every new request sees the current verifier.
14. Rate limiting is process-local transport state and cannot alter domain
    semantics; restart resets it.
15. Counters are fixed atomics; correlation and diagnostics are 64-entry rings;
    rate-limiter principal tracking is explicitly capped (1024 by default).
16. Logs/metrics never retain bearer values, Authorization headers, private
    keys, or credential content.
17. Shutdown diagnostics are best-effort local output. A closed diagnostic
    consumer does not block draining, Core owner release, or successful exit.
18. Adapter-owned collections are bounded. `tiny_http` parser pool/global
    queue and slow-client deadline remain `NOT_PROVEN` deployment boundaries.
19. A running Operation can remain nonterminal by domain design. Inspection
    exposes it; HTTP never fabricates a terminal state merely for diagnostics.
20. HTTP/Core/Protocol, authentication/authorization, recovery, error safety,
    ownership, offline Core, provider neutrality, and Windows loopback process
    execution are `PASS`. Cross-restart lease retention is `PARTIAL` (lease
    conflict/reacquisition semantics pass; an active lease spanning restart is
    not a dedicated process gate). Linux/macOS/TLS/public Internet remain
    `NOT_PROVEN`; MCP and SDK remain deferred.

## Frozen failure layers

| Layer | Examples | HTTP mapping |
| --- | --- | --- |
| Transport | refused, timeout, disconnect, malformed HTTP | client error or 4xx |
| Authentication | missing/invalid bearer | 401 |
| Authorization | forbidden Agent binding | 403 |
| Protocol | invalid envelope, revision/lease conflict, unknown Operation | 400/409 |
| Operation | accepted, running, completed, failed, cancelled, unknown | Protocol result; not inferred from HTTP 500 |
| Core | unavailable or restarting | 503 |

The diagnostic ring records `AUTH_REJECTED`, `AUTHZ_REJECTED`, `RATE_LIMITED`,
`MALFORMED_REQUEST`, `PROTOCOL_ERROR`, `REVISION_CONFLICT`, `LEASE_CONFLICT`,
`OPERATION_ACCEPTED`, `OPERATION_COMPLETED`, `OPERATION_FAILED`,
`UNKNOWN_OUTCOME`, `CORE_UNAVAILABLE`, and `SERVER_SHUTDOWN`.

## Deployment boundary

Plain HTTP is **LOCAL DEVELOPMENT / EXPLICITLY TRUSTED NETWORK ONLY**.
Remote production requires external TLS termination, connection/queue limits,
header/body/idle timeouts, and request deadlines. TLS, OAuth/OIDC/JWT, MCP,
rollback, cancellation, distributed limiting, SDK, production secret manager,
Linux/macOS, and public Internet deployment remain `NOT_PROVEN` or deferred.

## Verification

`tests/remote_failure_semantics.rs` proves failure-layer mapping, operation
failure semantics, recoverable auth/authz rejection, bounded rings, bounded
principal tracking, secret-safe diagnostics, and shutdown diagnostics.
M4-015's independent-process suite remains the real TCP/process evidence for
restart, reconnect, retry, revision, lease, credential lifecycle, ownership,
and failure isolation.

Protocol `hello` remains truthful: it advertises only the frozen command/query
set and provider/transport neutrality. It does not advertise TLS, rollback,
remote cancellation, distributed limiting, MCP, or an SDK.

## Readiness matrix

| Capability | Status |
| --- | --- |
| Remote HTTP Core / Protocol compatibility | PASS |
| Failure semantics | PASS |
| Unknown-outcome recovery | PASS for durable lost-response resolution |
| Retry idempotency / revision recovery | PASS |
| Lease recovery | PASS / PARTIAL; conflicts and reacquisition pass, dedicated cross-restart active-lease gate absent |
| Credential lifecycle | PASS at opaque verifier boundary |
| Adapter-owned resource bounds | PASS |
| tiny_http global parser queue / slow client | NOT_PROVEN |
| Observability | PASS / PARTIAL; bounded local diagnostics, no production exporter |
| Error safety / graceful shutdown / Core restart | PASS |
| Second-Core protection / offline Core / provider neutrality | PASS |
| Windows | PASS for supported process topology |
| Linux / macOS / TLS / public Internet | NOT_PROVEN |
| MCP / SDK | DEFERRED |
