# Idempotency

## Contract

Every command and mutating runtime request carries a caller-generated `request_id`. Pong stores a request outcome keyed by `(project_id, actor_id, request_id)` for the retention window. Repeating the request returns the original result if the canonical command digest matches.

## Safe retries

Retries are safe for metadata commands, blob uploads, event append, checkpoint creation, and commit publication when the same request ID and expected head are reused. A changed payload with an existing request ID returns `IDEMPOTENCY_KEY_REUSE`.

## Tool execution

Pong does not assume arbitrary tools are idempotent. The runtime uses provider idempotency keys, operation intent records, and precondition checks. If execution status is unknown, the API returns `SIDE_EFFECT_UNKNOWN`; the caller must inspect provider/workspace evidence before replay or compensation.

## Duplicate events

Event consumers deduplicate by `event_id` and `(stream_id, sequence)`. Duplicate delivery is permitted. Event payloads are immutable; corrections use compensating events linked by `supersedes` or `compensates`.

## Commit and ref updates

Commit IDs are content-derived from canonical metadata and parent IDs, so identical commits converge. Ref updates require expected old value and are idempotent when the new value is already current.

## Retention

Idempotency records remain at least as long as the maximum retry/recovery window and may be compacted only after an audit marker. Offline clients should persist request IDs across restarts.

