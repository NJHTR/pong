# SDK and Service API Design

**Status: Proposed contract.** Python SDK, local protocol bindings, and a future REST service are views over the same commands and projections. Core domain rules remain below all bindings.

## Python SDK shape

The public client is a context-bound object rather than a module-global singleton:

```text
PongClient.open(project, actor, transport, request_id_factory)
client.status(consistency="committed")
client.agents(cursor=None)
client.workspace(id=None)
client.current_branch()
client.current_commit()
client.diff(base, target)
client.log(scope, cursor=None)
client.inspect(kind, id)
client.checkpoint.create(request_id, ...)
client.commit.create(request_id, expected_head, ...)
client.branch.create(request_id, name, from_commit=None)
client.checkout(request_id, branch, expected_lease=None)
client.rollback.plan(request_id, target, dimensions)
client.rollback.apply(request_id, plan_id, approval)
client.replay.plan(request_id, operation_range)
client.replay.apply(request_id, plan_id, approvals)
```

Methods return typed result objects containing `status`, `warnings`, `observed_sequence`, and redaction metadata. They raise only transport/serialization exceptions; domain rejection is a typed result/error with stable `code`, so agents can branch safely. The SDK exposes no direct database or `.pong` mutation API.

## REST binding (future service)

The REST binding maps resources to protocol commands without changing semantics:

| Resource | Read | State-changing action |
| --- | --- | --- |
| `/v1/projects/{project}` | project/status | initialization is local-only until service mode |
| `/v1/projects/{project}/agents` | list/inspect | register/heartbeat |
| `/v1/projects/{project}/workspaces` | list/inspect | create/attach/detach/checkout |
| `/v1/projects/{project}/branches` | list/show | create/advance/merge |
| `/v1/projects/{project}/commits` | log/show/diff | create |
| `/v1/projects/{project}/checkpoints` | list/show | create/restore |
| `/v1/projects/{project}/operations` | inspect/activity | record/replay-plan/replay-apply |
| `/v1/projects/{project}/events` | cursor stream | append only through authorized commands |

Commands use `POST` with `Idempotency-Key` equal to protocol `request_id`; expected heads/lease epochs are explicit preconditions. Long-running work returns `202` plus a job/event cursor. Domain conflicts use a stable error body and `409`; policy denial `403`; uncertain effects `409` with `SIDE_EFFECT_UNKNOWN`; integrity/recovery `503` or a typed `RECOVERY_REQUIRED` response. JSON fields and pagination follow the protocol's version negotiation.

## Cross-binding guarantees

1. The same request ID and payload have one outcome across retries.
2. Reads declare consistency and return the observed sequence/source.
3. Every mutation is authorized, observable, and linked to an actor/session.
4. No binding can rewrite events, immutable objects, or historical refs.
5. Version negotiation, deprecation, redaction, and migration rules are shared.

