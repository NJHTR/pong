# Consistency Model

The lease/revision rules below are now exercised by the bounded internal M2
workspace slice. Commit, branch, event-linkage, and provider-reconciliation
semantics remain target contracts until their respective milestones pass.

## Streams and ordering

Each workspace, agent, and project has an append-only event stream with monotonic sequence numbers. Causal links (`parent_event_id`, operation parent, commit parent) define partial order across streams. Consumers must not infer a total order from wall-clock timestamps.

## Levels

- **Local durable**: data is fsynced to the local journal/object store.
- **Committed**: metadata transaction and referenced blobs are durable and visible through the branch ref.
- **Observed**: consumer has read through a specified sequence.
- **Reconciled**: runtime and workspace scan agree on the recorded state.

Queries declare a level; responses return the observed sequence and whether the level was met. Local-first operation favors local durable and committed consistency, not global linearizability.

## Atomicity

An operation event and its state-delta manifest are published atomically in metadata. Blob writes may precede publication, but unreachable blobs are garbage-collectable. A commit publishes only after all referenced snapshots and manifests verify.

## Concurrency

Branch refs use compare-and-swap. Workspace writes use an epoch lease by default; optimistic mode records precondition hashes and surfaces conflicts at commit. Two agents may safely read the same workspace; concurrent mutation requires explicit policy.

## Visibility

Events become visible after journal durability. Subscribers may receive duplicates and out-of-order cross-stream notifications; they use sequence/cursor and idempotency keys to converge.

## Conflict semantics

Conflicts are first-class records containing base, ours, theirs, and affected paths/resources. Auto-merge is limited to declared deterministic strategies. Unresolved conflicts block ref publication but do not delete either input history.
