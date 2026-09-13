# M4-007 Offline-First Core Architecture

**Status:** `PASS / INTERNAL / TEST-GATED`

Pong Core is the product itself: an offline-first, local-first, network-
independent Agent execution/state/control plane. Remote access is optional
adapter functionality and is never a Core startup or storage dependency.

## Frozen Boundary

```text
                    +------------------+
                    |  Agent Runtime   |
                    +--------+---------+
                             |
                    External Agent Protocol
                             |
                    +--------v---------+
                    |   AgentControl   |
                    +--------+---------+
                             |
                    +--------v---------+
                    |    Pong Core     |
                    | Agent            |
                    | Task             |
                    | Execution        |
                    | Workspace        |
                    | Version          |
                    | Snapshot         |
                    | Checkpoint       |
                    | Handoff / Resume |
                    | Rollback / Diff  |
                    | Restore          |
                    | Operation        |
                    | Lease / Revision |
                    +------------------+
                         ^       ^    ^
                         |       |    |
                      JSONL    HTTP  MCP
                      local   future adapter
```

Every transport can be absent while Core remains valid. The External Agent
Protocol is network-agnostic. MCP remains an optional adapter through that
protocol. Authentication is a remote security concern and does not enter Core
domain records.

## Offline Audit

`Cargo.toml` contains local Rust, SQLite (`rusqlite` bundled), filesystem,
serialization, hashing, and platform filesystem dependencies only. There is no
HTTP client/server, WebSocket, MCP runtime, cloud SDK, remote database, or
credential provider dependency. Repository initialization opens local SQLite
and local CAS directories; no network startup path exists in `src/`.

The offline E2E test uses only `tempfile`, local filesystem paths, local SQLite,
and the in-process Rust API. It does not spawn an external provider or transport
and does not access internet services.

## Capability Results

| Capability | Status | Evidence |
| --- | --- | --- |
| Network independence | PASS | Dependency/runtime audit and `offline_core_e2e`. |
| Local execution | PASS | Complete durable workflow in temporary local repository. |
| Durable reopen/recovery | PASS | Cold reopen verifies Version, Checkpoint, Handoff, Resume, Rollback. |
| Remote optionality | PASS | Core has no transport/security imports; adapters sit above Protocol. |
| MCP optionality | PASS / NOT_IMPLEMENTED | No MCP dependency or startup path; adapter remains deferred. |
| Provider neutrality | PASS | Core uses runtime-neutral Agent metadata only. |
| Production remote security | NOT_PROVEN | Authentication, TLS, rate limits, and deployment controls remain outside Core. |

## Local and Remote

Local execution may rely on a same-machine process, local filesystem, and
local database. Remote execution must add authentication, authorization,
reconnect, replay protection, timeout, audit, rate limiting, TLS/channel
security, and credential rotation at the transport/security boundary. These
requirements do not alter Core semantics.

## Offline Workflow

The tested local path is:

```text
Agent -> Task -> Execution -> Workspace -> Operation
      -> Snapshot/Version -> Checkpoint -> Handoff -> Resume
      -> Restore/Rollback -> cold reopen
```

The resulting state is durable without HTTP, WebSocket, MCP, cloud services,
remote authentication, or internet access. `Workspace.head`, Version and
Snapshot ownership, Operation outcomes, lease/revision checks, and history
remain governed by existing Core contracts.

## Deferred Work

Remote sync, HTTP/WebSocket transports, MCP adapter, SDKs, distributed
coordination, remote databases, and external authentication are optional future
layers. They must not become Core startup, storage, lifecycle, or execution
dependencies.
