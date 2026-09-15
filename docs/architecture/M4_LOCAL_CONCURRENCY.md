# M4-008 Local Multi-Runtime Concurrency

**Status:** `PARTIAL / INTERNAL / TEST-GATED`

This slice freezes local concurrency semantics. It does not define remote
transport or multi-Pong replication.

## Terms

- **State consistency:** durable invariants across Operations, Executions,
  Workspaces, Versions, Snapshots, and recovery records.
- **Process coordination:** serialization and conflict detection among local
  callers using one repository.
- **Synchronization:** callers refresh durable state after revision, lease, or
  uncertain-outcome conflicts.
- **Persistence:** SQLite WAL, CAS, and local filesystem durability.
- **Transport:** how a Runtime reaches one Core; not consistency authority.
- **Replication:** copying state between separate Pong Cores; deferred.

Multiple Runtimes using one long-lived Core is concurrent access. It is not
distributed replication.

## Actual Concurrency Model

Pong's Rust `Repository` handle is mutable and not a shared concurrent service.
A transport such as the current JSONL adapter owns one long-lived Repository
and serializes requests. Durable conflict semantics still protect callers that
observed old state or reconnect later.

Multiple independent Repository handles/processes use SQLite WAL and a shared
repository lock. Basic concurrent opens and Operation writes can succeed.
Windows Workspace/CAS publication is not reliably supported across independent
handles because repository startup scans race with SQLite byte-range locks and
temporary CAS/sidecar files. These attempts fail closed with Windows code 33
or transient code 2; no corruption or silent overwrite was observed.

## Ownership and Guards

- **Lease:** protects Workspace mutation. The durable owner is `agent_id`, not
  Runtime, process, connection, Principal, or Execution. Protocol authorization
  first validates Agent -> Execution -> Workspace binding.
- **Epoch/expiry:** lets another Agent take over after lease expiry and makes an
  old token permanently stale.
- **Revision/CAS:** protects Workspace, Execution, Task, and head transitions
  from stale observations. A stale write fails; it never overwrites R+1.
- **Operation identity:** makes an exact request replayable and rejects changed
  reuse. A different request/Operation may represent the same business intent;
  Operation is not global semantic deduplication.
- **SQLite transaction:** serializes durable metadata mutation.
- **CAS digest:** validates immutable content identity; it is not a writer
  coordination primitive by itself.

## Concurrency Matrix

| Caller | Execution | Workspace | Action | Result |
| --- | --- | --- | --- | --- |
| A | E1 | W1 | read | Allow; no lease required for read-only query. |
| A | E1 | W1 | write with valid lease/revision | Allow. |
| B | E2 | W2 | write with valid lease/revision | Allow independently. |
| A | E1 | W1 | stale write | Reject with conflict; refresh and retry explicitly. |
| B | E2 | W1 | write while A lease active | Reject with lease conflict. |
| A | E1 | W1 | checkpoint | Allow only for valid durable bindings; immutable exact retry. |
| B | E2 | W1 | checkpoint | Requires its own valid Execution/Workspace/Version bindings; ID knowledge is insufficient. |
| A | E1 | W1 | rollback | Requires target lease and expected revision. |
| B | E2 | W1 | rollback while A owns lease | Reject; no last-writer-wins. |

Different Workspaces are logically independent. Same Workspace writers are
coordinated by lease, epoch, revision, and CAS checks.

## Crash and Retry Semantics

1. A started Operation survives Runtime loss and is inspectable after reopen.
2. A lease holder crash is recovered by expiry and a higher epoch; manual row
   deletion is not required.
3. Checkpoint/Handoff/Resume identities are durable and exact retries do not
   create duplicate semantic records.
4. Rollback uses prepared/completed recovery and lease/revision authority.
5. An uncertain request resolves through durable request/Operation identity.

## Test Evidence

`tests/local_concurrency.rs` contains active tests for lease contention and
expiry takeover, stale Execution revision, Operation replay/reuse rejection,
cold-reopen Operation state, real multi-process open, metadata writers, and
Workspace publication. Existing active suites cover concurrent Execution CAS,
Checkpoint/Handoff replay, parallel rollback isolation, reconnect, and
publication recovery.

Windows stress observations:

- 20 multi-process Workspace-publication rounds: 0 diagnostic-test failures,
  13 rounds with code 33 sharing contention, 2 rounds with transient code 2.
- Existing three-Workspace control test: 5 rounds, 4 passes and 1 code 33
  failure.
- All observed failed writers were fail-closed; successful writers were durable
  and unopened Workspaces retained their old heads.

## Status

| Capability | Status |
| --- | --- |
| Local multi-Runtime through one coordinating Core | PASS |
| Local multi-Runtime overall | PARTIAL |
| Workspace concurrency semantics | PASS; independent-process availability PARTIAL on Windows |
| Execution concurrency | PASS |
| Operation idempotency | PASS |
| Lease recovery | PASS |
| Revision/CAS stale-write protection | PASS |
| Crash recovery | PASS |
| Multi-process metadata operations | PASS in focused Windows run |
| Multi-process Workspace/CAS publication | PARTIAL / WINDOWS IMPLEMENTATION LIMITATION |
| Linux/macOS multi-process parity | NOT_PROVEN in this slice |
| Remote sync / multi-Pong replication | DEFERRED |

## Required Next Boundary

Before claiming general multi-process support, Pong must choose and test one of
two explicit operational contracts: a single local Core process that brokers
all Runtime access, or hardened multi-handle repository startup/scanning that
is safe with active SQLite/CAS writers on every supported platform. This is not
a reason to add HTTP, MCP, CRDT, Raft, or replication.
