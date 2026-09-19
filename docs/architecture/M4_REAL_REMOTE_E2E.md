# M4-015 Real Remote E2E

**Status:** `PASS / WINDOWS PROCESS-GATED / DEPLOYMENT PARTIAL`

**Baseline:** `f8b4212510594133c1fe584172d0867117dd8ee9`
(`checkpoint: m4-http-production-hardening`)

M4-015 proves that independent client processes can operate one Pong Core
through real loopback TCP and HTTP. It adds no business capability, protocol
field, Core schema, provider behavior, or alternate authorization system.

## Proven Topology

```text
independent pong-agent-http-client processes
  -> real 127.0.0.1:<dynamic-port> TCP
  -> independent pong-agent-http process
  -> HTTP parsing and credential verification
  -> per-request RemoteAccessBoundary session
  -> unchanged External Agent Protocol v1.0
  -> shared AgentProtocolCore dispatcher
  -> one Core-owned Repository
```

`tests/real_remote_e2e.rs` never calls the HTTP handler, dispatcher, or Core
from the test process. Normal, malformed-body, malformed-HTTP, and lost-response
traffic is emitted by `pong-agent-http-client` child processes. The validation
client is private to its binary, is not exported from `pong_core`, and is not
an SDK.

## Read-Only Audit Conclusions

1. `pong-agent-http` parses explicit Repository, Workspace root, credential,
   listen, and resource-limit arguments, then starts one `HttpRemoteServer`.
2. It obtains ownership with `Repository::open_as_core_owner` before binding
   the listener.
3. That process holds the sole Core owner for its lifetime; a second process
   using the same Repository exits with `CONFLICT`.
4. All fixed HTTP protocol workers share one `Arc<AgentProtocolCore>` and its
   single mutex-protected Repository; workers do not open Repository handles.
5. stdin `shutdown` stops acceptance, joins workers, drops the dispatcher and
   Repository, and releases the OS owner lock. Forced termination relies on OS
   lock release and normal cold-reopen recovery.
6. M4-013 already had test-local TCP helpers. M4-015 adds a separate
   `pong-agent-http-client` process so the client and server are different OS
   processes.
7. M4-013 and M4-014 tests use real `TcpStream` traffic, but many construct the
   server in the Rust test process. M4-015 process tests cross both process and
   TCP/HTTP boundaries.
8. No test calls the private `handle_request` function. Some retained HTTP
   tests construct `HttpRemoteServer` or a dispatcher in-process; they remain
   contract tests and are not counted as M4-015 process E2E evidence.
9. The test starts `CARGO_BIN_EXE_pong-agent-http` with only temporary paths and
   `--listen 127.0.0.1:0`.
10. Each case initializes a new Repository in a `tempfile::TempDir` and records
    only its synthetic Environment fixture.
11. A separate temporary Workspace root is passed with `--workspace-root`;
    protocol bindings are constrained child names below that root.
12. Credentials are passed by temporary regular JSON files. Secrets never
    appear in process arguments, metrics, errors, or retained evidence.
13. Repository, Workspace, and credential files all live in temporary
    directories, so user Pong data, the working Repository, and user home are
    not used.
14. Port zero lets the OS select a free loopback port. The server reports the
    actual address only after successful bind.
15. Readiness requires a bounded JSON control line and is followed by a real
    authenticated HTTP request; stdout alone is not treated as readiness.
16. Child exit is checked with bounded `try_wait` loops. Every started process
    is waited, including startup failures and fallback termination.
17. Core restart stops or forcibly terminates the first real server process,
    waits for exit, then launches a replacement against the same Repository.
18. A second `pong-agent-http` process against the live Repository must exit
    code 2 with a safe `CONFLICT`, while Core A continues serving requests.
19. Graceful shutdown is issued through the local stdin control channel; the
    old TCP endpoint closes, ownership releases, and a replacement opens the
    Repository normally.
20. Drop guards first close child stdin and wait for normal exit. They use a
    bounded kill-and-wait fallback only when a child fails to terminate, so no
    process or port is intentionally left behind.

## Process-Gated Behavior

The eight active M4-015 tests prove on Windows loopback:

- valid opaque credentials succeed and invalid/revoked credentials return 401;
- a Principal asserting an ungranted Agent returns 403, after which a valid
  client still succeeds;
- malformed HTTP, malformed Protocol JSON, and oversized bodies do not kill
  the server or corrupt the Repository;
- 400, 401, 403, 409, and safely induced 500-class responses expose no
  credential, filesystem path, SQLite text, Rust path, stack trace, or raw OS
  error;
- exact Operation retry returns one durable result;
- stale revision returns `REVISION_CONFLICT`, then refresh and retry succeed;
- a competing writer receives the existing `LEASE_CONFLICT`;
- three independent client processes concurrently use one HTTP process, one
  Core dispatcher, and one Repository;
- disconnect after sending does not cancel durable work; a fresh connection
  resolves the Operation by request identity;
- abrupt Core restart preserves Execution, Operation, Version, Checkpoint,
  Handoff, Resume, and continuation state;
- graceful shutdown closes the listener and releases Core ownership;
- credential replacement revokes A, accepts B, and leaves B's running
  Execution and Operation intact;
- request correlation remains transport-local and includes request,
  Operation, Principal, Agent, Execution, and Workspace identity without
  Authorization or bearer values;
- captured process diagnostics for exercised rejection paths contain no
  credential or Authorization value;
- request-body, response-size, per-principal rate, and fixed protocol-worker
  limits are active in the real process.

The real lifecycle is:

```text
Agent registration -> Task -> Execution A -> Workspace A -> Operation
  -> Snapshot/Version A -> current Version -> Checkpoint A -> Interrupt
  -> Resume Execution B -> Handoff -> Workspace B materialization
  -> continuation Version B -> Checkpoint B -> Core restart -> durable queries
```

Rollback is `NOT_PROVEN / PROTOCOL CAPABILITY GAP`. Pong has internal rollback
semantics, but External Agent Protocol v1.0 intentionally has no rollback
operation. M4-015 does not alter the frozen protocol to manufacture this proof.

## Transport Equivalence

Independent HTTP and JSONL process scenarios execute the same Protocol v1.0
operations and compare protocol-level status, result kind, stable identities,
revisions, and error details. The matrix covers success, unsupported-version
failure, exact retry, revision conflict followed by refresh, lease conflict,
missing Operation, process reconnect, and durable Operation resolution. HTTP
status and JSONL framing are deliberately excluded from the comparison.

## Resource And Deployment Boundary

The Pong protocol-handler count is fixed and observable, but it is not a global
connection limit. `tiny_http` still owns a dynamically growing parser pool and
an adapter-unconfigurable queue. It also exposes no safe socket read/idle
deadline. Consequently global connection/queue bounding and slow-client
deadlines remain `NOT_PROVEN` and require production ingress controls.

Plain HTTP remains **LOCAL DEVELOPMENT / EXPLICITLY TRUSTED NETWORK ONLY**.
Production remote deployment **REQUIRES EXTERNAL TLS TERMINATION** plus
connection limits, bounded queues, header/body/idle timeouts, and request
deadlines. TLS deployment, public Internet deployment, production secret
manager integration, Windows credential ACL validation, Linux parity, macOS
parity, and a production metrics/log exporter remain `NOT_PROVEN`.

## Status Matrix

| Capability | Status |
| --- | --- |
| Real independent-process E2E | PASS on Windows loopback |
| HTTP transport | PASS |
| External Agent Protocol v1.0 | PASS / unchanged |
| Authentication | PASS at opaque-credential adapter boundary |
| Authorization | PASS |
| Core ownership / second-Core rejection | PASS |
| Core restart / reconnect | PASS |
| Retry / idempotency | PASS |
| Revision / lease conflicts | PASS |
| Checkpoint / Handoff / Resume continuation | PASS |
| Rollback over Protocol v1.0 | NOT_PROVEN / PROTOCOL CAPABILITY GAP |
| Failure isolation / error safety | PASS at HTTP and handler boundary |
| Body, response, rate, protocol-worker limits | PASS |
| Global connection / parser queue bound | NOT_PROVEN |
| Slow-client deadline | NOT_PROVEN |
| Offline Core / provider neutrality | PASS |
| Windows | PASS for the supported process topology |
| Linux / macOS | NOT_PROVEN |
| TLS / public Internet | NOT_PROVEN |
| MCP | OPTIONAL ADAPTER / DEFERRED |

## Validation

The focused process suite passes 8 active tests with no ignored tests. The
HTTP hardening, HTTP remote transport, External Agent Protocol, remote contract,
protocol reconnect, Core ownership, local broker, Repository access-policy,
and offline-Core suites pass 77 active tests with no failures or ignored tests.

The aggregate `cargo test --all --locked` rerun reached the unchanged
`control_layer::three_provider_metadata_values_publish_to_isolated_workspaces`
direct-handle diagnostic and one parallel Repository open failed closed with
Windows raw OS code 33 (sharing violation). Its unchanged isolated rerun
passed. M4-015 uses the supported single-Core topology and did not weaken,
serialize, ignore, or delete that M4-008 diagnostic. The aggregate result is
therefore retained as `PARTIAL / PRE-EXISTING WINDOWS ENVIRONMENT`.
