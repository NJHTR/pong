# ADR-M4-009: Single Long-Lived Core Repository Ownership

- **Status:** `Accepted / Internal M4`
- **Date:** 2026-09-15

## Context

M4-008 proved the lease, revision, Operation, and recovery semantics required
for local concurrency. It also demonstrated that independent Windows processes
opening and publishing through the same Repository are not a reliable normal
runtime model: startup scans intermittently meet sharing violation code 33 or
transient code 2 during active SQLite/CAS publication.

The architecture therefore needs an explicit answer to who owns a live Pong
Repository. SQLite concurrency alone does not answer that authority question.

## Decision

Adopt a single long-lived Core owner as the normal external-Runtime contract.
The Core exclusively owns the live Repository, MetadataStore, SQLite
connection, CAS access, and filesystem mutation boundary. Agent Runtimes use
the external Agent protocol and do not directly open storage.

Implement a separate OS-held `core-owner.lock`. It is distinct from the
repository migration fence and from Agent-owned Workspace leases. The local
JSONL Core entry point acquires exclusive ownership for its lifetime. A second
Core or new direct Repository handle fails closed with `CONFLICT`.

The low-level Repository API remains available for Core implementation,
embedded use, tests, and offline maintenance. Shared embedded handles are only
available while no Core owner is active; they are not the supported concurrent
entry point for external Agent Runtimes.

## Recovery

The owner lock contains no durable state and is not a PID file. Process exit,
including forced termination, releases it through the operating system. A new
Core then reopens the durable Repository, runs existing recovery, and accepts
reconnected Runtime requests. No process memory or transport session becomes
part of Agent identity or repository truth.

## Consequences

- One Repository has one live state authority.
- Multiple Runtime identities may be coordinated through that authority.
- Core ownership does not weaken lease, epoch, revision, or Operation checks.
- Offline-first remains intact; a local coordinator does not imply Internet or
  a remote service.
- M4-008 direct Windows multi-process access remains `PARTIAL`; it is not
  relabeled as a successful Core workflow.
- Linux/macOS owner-lock parity and simultaneous multi-connection local
  transports remain `NOT_PROVEN`.
- HTTP, MCP, replication, CRDT, consensus, distributed locks, and cloud
  synchronization remain outside this decision.
