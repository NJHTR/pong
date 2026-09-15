# ADR-M4-008: Local Multi-Runtime Concurrency

- **Status:** `Accepted partial boundary / Internal M4`
- **Date:** 2026-09-15

## Decision

Define local concurrency around one Pong Core and its durable guards:
Workspace lease/epoch/expiry, optimistic revision/CAS, SQLite transactions,
immutable CAS identity, and Operation idempotency. Multiple Agent Runtimes may
use the same coordinating Core. Multiple Pong Cores and replication are out of
scope.

Lease ownership remains Agent-local. Execution identity does not own the lease,
but protocol authorization validates the Agent/Execution/Workspace relation
before Workspace authority is granted. A stale Runtime must refresh durable
state after `REVISION_CONFLICT` or uncertain outcome.

## Multi-Process Boundary

Independent local processes may open the same repository and basic metadata
writes can serialize through SQLite WAL. On Windows, concurrent Repository
startup and Workspace/CAS publication intermittently fail closed with sharing
violation code 33 or transient file-not-found code 2 while repository-owned
byte scanning observes active SQLite/CAS files.

Therefore general multi-process Workspace mutation is `PARTIAL`, not `PASS`.
No corruption or silent overwrite was observed. The existing failing control
test is retained; no test is disabled or weakened.

## Consequences

The supported architecture is currently one long-lived coordinating Core per
repository, with multiple Runtime requests serialized at that boundary and
durable conflict/reconnect semantics above it. Direct concurrent repository
handles require a later hardening slice or an explicit single-writer process
contract. Remote transport, replication, CRDT, consensus, distributed
transactions, Redis, and Kafka remain deferred.
