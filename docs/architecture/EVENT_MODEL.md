# Event Model

## Purpose

Events are immutable observed facts and lifecycle transitions. They support audit, projections, recovery, and collaboration views. An event is not a command and does not imply that a requested action succeeded.

## Envelope

Every event contains:

`event_id`, `project_id`, `event_type`, `schema_version`, `occurred_at`, `recorded_at`, `agent_id` (optional), `workspace_id` (optional), `task_id` (optional), `operation_id` (optional), `parent_event_ids`, `local_sequence`, `payload`, `redaction_status`, and `capture_confidence`.

The durable SQLite envelope names the project-local total-order field
`project_sequence` (the persisted spelling of the conceptual `local_sequence`)
and also retains a per-stream `sequence` for stream-local reads. Consumers use
`project_sequence` for cursor pagination and never infer order from timestamps.

Payloads are typed and versioned. Unknown fields must be ignored by readers; incompatible changes require a new schema version.

## Event classes

- operation: started, completed, failed, cancelled, unknown
- state: snapshot created, checkpoint created, commit created, ref moved
- lifecycle: agent registered, heartbeat, workspace leased, environment drift
- collaboration: message sent, handoff, artifact linked, merge requested
- recovery: journal replayed, reconciliation required, restore completed
- policy: permission denied, approval requested, secret redacted

## Ordering and delivery

Events are append-only and durable before acknowledgement. Local sequence provides per-project total order in v0.x. Parent links provide causality across agents. Delivery to projections or external consumers is at-least-once; consumers deduplicate by event id.

## Event versus operation

An operation is an attempted action and owns result semantics. Events describe the operation lifecycle and related facts. One operation may produce many events; a lifecycle event may exist without an operation. This separation allows failed and partially observed actions to remain visible.

The bounded M3 ledger currently writes `started` and terminal operation events
to an `operation:<operation_id>` stream in the same SQLite transaction as the
operation row and its existing intent/outcome journal record. Runtime capture
and external delivery remain future work.

## Retention and privacy

Events may reference object or artifact digests instead of embedding large or sensitive data. Redaction status is immutable metadata. Deletion or compaction requires a documented retention policy and must preserve referential integrity for retained commits and checkpoints.
