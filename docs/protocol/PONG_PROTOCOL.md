# Pong Protocol

## Purpose and scope

Pong Protocol is the wire-neutral contract between a Pong client, the local runtime, storage adapters, and optional remote services. It describes identity, envelopes, ordering, errors, and capability negotiation. It does not prescribe HTTP, IPC, or a programming language; bindings may use JSON-RPC, REST, Unix sockets, or an SDK while preserving these semantics.

Pong is infrastructure for versioning agent execution. It records workspace state, operations, events, artifacts, and collaboration context. It does not perform reasoning, planning, scheduling, prompting, or model selection.

## Protocol layers

1. **Core protocol**: identifiers, event/operation envelopes, refs, snapshots, commits, and capability negotiation.
2. **Transport binding**: serialization and delivery (local file, socket, HTTP, or embedded call).
3. **Runtime protocol**: interception and lifecycle messages emitted by an execution environment.
4. **Agent protocol**: explicit queries and state-changing commands for agents and humans.

Every message carries `protocol_version`, `project_id`, `request_id`, and an authenticated `actor`. Unknown additive fields must be ignored by readers; incompatible changes require a new major protocol version and an ADR.

## Canonical envelope

```json
{
  "protocol_version": "0.1",
  "message_type": "operation.recorded",
  "message_id": "msg_01J...",
  "request_id": "req_01J...",
  "occurred_at": "2026-08-19T10:00:00Z",
  "project_id": "prj_...",
  "actor": {"type": "agent", "id": "agt_..."},
  "workspace_id": "ws_...",
  "branch": "main",
  "payload": {},
  "trace": {"parent_message_id": null, "correlation_id": "..."},
  "integrity": {"hash": "sha256:...", "prev_hash": "sha256:..."}
}
```

`message_id` is globally unique; `request_id` is unique for a client attempt and is the idempotency key for commands. `occurred_at` is the producer's time, while the local event store also assigns a monotonic sequence. Hashes cover the canonical serialized envelope and make tampering detectable, not impossible.

## Identifiers and clocks

Identifiers are opaque, URL-safe strings with a type prefix (`agt_`, `ws_`, `op_`, `evt_`, `cmt_`, `snp_`, `art_`). Clients must not infer ordering from IDs. Ordering is defined by `(project_id, stream_id, sequence)`; cross-stream order is represented by causal links and may be concurrent. Producers should provide a monotonic logical counter and an optional wall clock; consumers tolerate clock skew.

## Core message types

- `operation.started`, `operation.finished`, `operation.failed`: tool execution lifecycle.
- `event.appended`: durable domain event notification.
- `snapshot.created`: immutable state manifest.
- `commit.created`, `ref.updated`: version graph changes.
- `checkpoint.created`: resumable execution marker.
- `artifact.registered`: content-addressed artifact metadata.
- `agent.presence`, `task.updated`, `message.posted`: collaboration context.
- `command.request`, `command.accepted`, `command.rejected`, `command.completed`: request/response for explicit APIs.

Commands MUST be acknowledged before completion when work can be long-running. A completion includes `status`, `result`, `warnings`, and `observed_sequence` so a caller can resume from a known point.

## State transitions

State-changing operations follow `validate -> authorize -> reserve -> execute -> record -> publish`. A failed validation or authorization produces no state mutation. If execution succeeds but recording fails, the runtime writes an outbox item and marks the operation `unreconciled`; recovery later reconciles it. No consumer may treat an unrecorded operation as committed.

## Errors

Errors are stable objects: `code`, `message`, `retryable`, `details`, and `safe_to_expose`. Codes include `AUTH_REQUIRED`, `FORBIDDEN`, `NOT_FOUND`, `CONFLICT`, `STALE_HEAD`, `INVALID_STATE`, `UNSUPPORTED`, `INTEGRITY_ERROR`, `RECOVERY_REQUIRED`, `SIDE_EFFECT_UNKNOWN`, and `RESOURCE_EXHAUSTED`. Clients branch on `code`, never on prose. Retryable commands must reuse the same `request_id`.

## Capability negotiation

Peers exchange `protocol_version`, supported message types, storage features, maximum payload size, redaction profile, and consistency guarantees. A feature is used only when both sides advertise it. Local v0.x advertises append-only events, CAS blobs, branch refs, operation journaling, redaction hooks, and single-writer metadata transactions; distributed leases and remote merge orchestration are optional capabilities.

## Compatibility rules

Minor versions add fields or message types. Producers preserve old required fields for one deprecation window. Readers ignore unknown fields and retain unknown payloads as opaque data. A major upgrade requires migration notes, fixture updates, and an ADR. Protocol records are append-only; correction is represented by a compensating event, never by rewriting history.

## Security and privacy

The envelope contains only identifiers and policy-approved metadata. Secrets, raw credentials, and unrestricted environment variables are forbidden. Payloads pass through a redaction policy before persistence and transport. Integrity hashes authenticate content only when keys are configured; authorization is a separate capability decision (see `docs/security`).

## Reference invariants

- A commit references immutable snapshot IDs and has one or more parent commits.
- A ref update is compare-and-swap against the caller's expected old value.
- Every operation has exactly one outcome (`succeeded`, `failed`, `cancelled`, or `unknown`); recording state may additionally be `unreconciled` until publication is repaired.
- Event sequence numbers are unique within a stream and never reused.
- Deleting a ref does not delete reachable objects.
- Replay and rollback always create new events; they do not mutate historical events.
