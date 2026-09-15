# M4-009 Single Long-Lived Core Ownership

**Status:** `PASS / INTERNAL / POLICY-FROZEN`

Pong defines one state authority for a live local repository: one long-lived
Core owner. Multiple Agent Runtimes reach that owner through the external
Agent protocol. This remains an offline-first local process architecture; it
does not require a network service.

## Ownership Boundary

The durable repository is the `.pong` namespace plus Workspace filesystem
state. A `Repository` instance opens and owns:

- one `MetadataStore` and its SQLite connection;
- the filesystem CAS and its staging/quarantine directories;
- repository compatibility and migration metadata;
- access to Workspace mutation, Operation lifecycle, lease, and revision APIs.

`WorkspaceManager`, `AgentControl`, and `ExternalAgentProtocol` borrow that
Repository. They do not create a second storage authority. The local JSONL
process now opens the Repository as its Core owner and retains ownership for
the process lifetime.

Agent Runtimes own Agent behavior and protocol requests. They do not own the
SQLite connection, CAS, or repository filesystem. Agent identity, Execution
identity, Workspace lease, transport connection, and Core process identity are
separate concepts.

## Model Comparison

| Dimension | Model A: shared direct handles | Model B: single Core owner |
| --- | --- | --- |
| Correctness | Durable guards fail closed, but startup scans can contend with active writers. | One live storage authority; all Runtime mutations pass through the same control boundary. |
| Concurrency | SQLite and filesystem activity occur behind independent startup/open paths. | Core serializes current JSONL requests; lease and revision still reject stale logical callers. |
| Windows | Workspace/CAS publication is `PARTIAL`; code 33 and transient code 2 are observed. | Owner exclusion, workflow, forced-exit recovery, and reconnect pass on Windows. |
| Linux/macOS | Multi-process parity is `NOT_PROVEN`. | Owner-lock parity is `NOT_PROVEN`. |
| Recovery | Each process independently invokes startup recovery. | Replacement Core invokes startup recovery once, then Runtimes reconnect. |
| Crash handling | A failed writer is isolated, but another handle may already be active. | OS lock release permits a replacement owner; durable state is independent of process memory. |
| Lease | Agent-owned Workspace lease still applies. | Same; Core ownership does not replace Agent lease. |
| Operation | Durable request/Operation identity is shared through SQLite. | Same ledger, reached through one authority; no second idempotency system. |
| Performance | Repeated Repository open, scan, SQLite, and CAS initialization. | One startup scan and long-lived stores; request handling avoids per-Runtime reopen. |
| Isolation | Direct callers can bypass `AgentControl`. | External Runtime writes pass through protocol and control authorization. |
| Lifecycle | No unique live owner lifecycle. | Startup, serve, terminate, reopen, recover, reconnect. |
| Security | Repository path possession can become storage authority. | Transport/authentication may establish a Principal before AgentControl without exposing storage. |
| Complexity | Cross-process storage interactions remain part of every Runtime. | One small owner fence; no replication or distributed coordination. |
| Future remote transport | Remote and local clients could diverge in authority. | Replacing transport does not change Core ownership or domain semantics. |
| Provider neutrality | Possible, but providers may embed storage access. | Runtime/provider metadata stays outside Core authority. |
| Offline-first | Local but operationally multi-writer. | Fully local and network-independent; a local coordinator is not a cloud dependency. |

**Recommended and frozen normal-runtime model: Model B.**

## Lock Contract

Pong uses three different authorities:

1. `core-owner.lock` defines which live Core process owns a Repository.
2. `repository.lock` coordinates repository namespace/migration access.
3. Workspace lease/epoch/expiry defines which Agent may mutate a Workspace.

They are not interchangeable.

Ordinary embedded `Repository` handles take a process-refcounted shared
Core-access lock. A Core
owner first completes normal repository health/open checks, then releases its
temporary shared access and attempts an exclusive owner lock. If another
handle enters during that upgrade boundary, exclusive acquisition fails and
the Core does not start. Once ownership is held:

- a second Core startup returns `CONFLICT`;
- a new direct `Repository::open` returns `CONFLICT`;
- offline migration remains blocked by the repository namespace lock;
- an Agent still needs the correct Execution/Workspace binding and lease;
- stale revisions still fail instead of overwriting durable state.

The lock is an OS-held file lock, not a PID file. Normal drop and forced
process termination release it without manual cleanup. It contains no durable
domain state and is excluded from repository secret-byte scanning.

## Multi-Runtime Semantics

The protocol may carry requests for multiple Agent identities through one
Core connection. Connection identity is not Agent identity. The current JSONL
transport processes requests sequentially, while durable Operation identity,
lease, epoch, and revision preserve retry and stale-caller behavior across
disconnect/reconnect.

The active process E2E proves two Runtime identities complete checkpoint,
handoff, resume, materialization, continuation, and a second checkpoint through
one Core. The Core is then forcibly terminated. A replacement Core reopens the
same repository and exposes the complete durable Execution state.

Multiple simultaneous client connections to one live Core are a future local
transport concern. They do not require multiple Repository owners.

## Crash Recovery

Core process state is disposable. Agent, Task, Execution, Operation, Workspace,
Version, Snapshot, Checkpoint, Handoff, Resume, Rollback, revision, and lease
records remain governed by SQLite transactions, filesystem publication, CAS
identity, and existing recovery procedures.

After Core termination:

1. the OS releases `core-owner.lock`;
2. a replacement Core performs normal integrity and unfinished-operation recovery;
3. Runtimes reconnect with opaque durable IDs;
4. protocol inspection resolves completed, failed, started, or unknown outcomes;
5. retries use the existing request/Operation identity contract.

## Local, Remote, and Replication

Local access is Runtime -> local transport -> Core owner -> Repository. A
future remote adapter may add authentication and authorization before the same
protocol/Core boundary. Core ownership does not change.

Multi-Pong replication is a separate future problem and remains deferred. This
slice adds no HTTP, MCP, cloud dependency, replication, consensus, distributed
transaction, remote database, or schema migration.

## Verified Status

| Capability | Status |
| --- | --- |
| Repository ownership contract | PASS on Windows |
| Single Core owner exclusion | PASS on Windows |
| Multi-Runtime identities through one Core | PASS |
| Second Core rejection | PASS on Windows |
| Direct open rejection while Core is live | PASS on Windows |
| Core forced-exit lock recovery | PASS on Windows |
| Durable workflow after Core termination | PASS |
| Offline Core | PASS |
| Model A direct multi-process Workspace access | PARTIAL / IMPLEMENTATION LIMITATION |
| Linux/macOS Core-owner lock parity | NOT_PROVEN |
| Multiple simultaneous transport connections | NOT_PROVEN / TRANSPORT BOUNDARY |
| Remote transport compatibility | PASS at contract level; transport DEFERRED |

M4-011 now freezes Model B as the supported external Runtime path and retains
Model A as internal/embedded only. The final policy validation passed the full
regression, including active M4-008 direct-access diagnostics. Historical
Windows code `2`, `5`, and `33` observations remain recorded; they do not widen
Mode C into a supported external access path.
