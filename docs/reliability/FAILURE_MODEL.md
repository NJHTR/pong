# Failure Model

## Failure classes

- **Validation**: malformed command or impossible state; no mutation.
- **Authorization**: denied capability or approval; no execution.
- **Tool**: tool returns an error, timeout, or cancellation; operation is failed with evidence.
- **Observation**: tool executes but interception is partial; operation is degraded or unreconciled.
- **Process/host**: crash, power loss, disk full, or network loss; journal recovery applies.
- **Consistency**: stale head, lease loss, duplicate request, or conflicting merge; caller must re-read.
- **Integrity**: hash/signature mismatch or corrupt blob; quarantine and stop publication.

## Operation outcomes

An operation has one outcome: `succeeded`, `failed`, `cancelled`, or `unknown`. It also has a recording state; `unreconciled` means execution evidence exists but durable event publication is incomplete. `unknown` means execution may have happened; it is never treated as safe to retry blindly. Keeping outcome and recording state separate prevents a successful tool call with a missing event from being mistaken for a failed tool call.

## Blast-radius policy

Failures are contained to a workspace and stream where possible. A corrupt object blocks only descendants that require it; unrelated branches remain readable. Security or integrity failures fail closed and require operator action.

## Timeouts and cancellation

Timeouts are advisory unless the runtime can prove process termination. Cancellation records intent and outcome separately. External calls use provider idempotency keys where supported; otherwise they remain `unknown` after transport loss.

## Degraded mode

When storage or event publication is unavailable, local journaling may continue within quota. New irreversible operations are blocked unless policy explicitly permits degraded execution. Status exposes degraded mode and pending reconciliation count.
