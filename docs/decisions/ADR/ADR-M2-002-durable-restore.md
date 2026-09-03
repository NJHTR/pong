# ADR-M2-002: Durable Snapshot Restore

**Status:** Proposed / internal M2 slice

## Context

M2-SLICE-001 publishes an immutable, verified snapshot as a CAS tree plus
durable metadata. Restore must materialize that state without treating a
filesystem path as identity or overwriting user data. The existing local
materializer already provides temporary-tree construction and atomic
no-replace publication, but the restore intent and result also need to survive
restart and provider failures.

## Goals and Non-goals

Goals are a local restore operation, durable operation/event linkage, explicit
completed/failed/unknown outcomes, cold-reopen inspection, and idempotent retry.
Diff, restore into an existing workspace, commits/branches, CLI/SDK, remote
providers, and public API freeze are out of scope.

## Identity and Validation

The source is a durable `SnapshotRecord`; its `snapshot_id` must equal
`snp-<root-digest>`, its manifest and blob objects must verify in the expected
CAS domains, and its workspace/project/generation/redaction identities must
match the opened repository. The destination is a canonical local directory
outside `.pong`. Existing destinations are conflicts for a first request and
are never deleted or overwritten.

Restore operation identity is the caller-supplied `operation_id` plus the
existing project/agent/request idempotency tuple. The restore event has a
distinct event ID and carries operation, workspace, snapshot, causation,
correlation, generation, and migration identities.

## Atomicity Boundary

The filesystem side first builds a temporary tree, verifies every CAS object,
and publishes it with an atomic no-replace rename. Only after publication is
verified does Pong commit the terminal operation outcome and domain event in
one SQLite transaction. Workspace head/revision is not changed by restore.

## Failure and Retry

Before publication, failures clean the matching temporary tree and persist a
`failed` operation when possible. A destination-parent sync failure after the
rename is represented as `unknown` while retaining the destination. A SQLite
failure after materialization leaves the operation `started`; an exact retry
verifies the existing destination and records the terminal result without
overwriting it. Completed and unknown retries verify the destination and return
the original durable result. Terminal results cannot be rewritten with a
different snapshot or destination.

## Recovery and Security

Cold reopen reads the operation/event and revalidates the destination against
the canonical manifest and immutable blobs. Unsafe paths, links/reparse
points, extra entries, missing objects, wrong-domain objects, malformed
manifests, and identity mismatches fail closed. Restore never implies reversal
of external side effects.

## Compatibility

The operation ledger and event envelope are reused without changing the v0.1
reader contract. Restore-specific values are additive internal M2 behavior;
the ADR remains proposed and is not a public API acceptance.
