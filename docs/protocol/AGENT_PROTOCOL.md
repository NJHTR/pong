# Agent Protocol

## Role

The Agent Protocol is the explicit API used by an agent or human to inspect and change Pong state. It is intentionally narrower than a framework API: Pong exposes identity, workspace, version, execution history, and collaboration context, while the framework retains planning and orchestration.

## Session and identity

An agent establishes a session with `agent.register` or `agent.attach`, presenting an agent ID (or requesting one), project ID, workspace ID, framework name/version, and a capability token. Pong returns a session ID, granted capabilities, current branch/head, and a redaction profile. Heartbeats update presence but do not imply liveness of the underlying process.

An agent may own multiple workspaces, but a workspace has one active writer lease by default. Shared read access is normal; shared writes require an explicit collaboration policy and conflict handling.

## Read operations

The minimum query surface is:

`status`, `agents`, `agent.inspect`, `workspace.list`, `workspace.inspect`, `branch.list`, `branch.current`, `log`, `diff`, `show`, `inspect`, `activity`, and `capabilities`.

Queries accept a consistency hint (`local`, `committed`, or `fresh`) and a cursor. Results include `observed_sequence` and `source` so an agent can tell whether it observed a local cache or a reconciled stream.

## Write operations

Commands are `workspace.create|attach|detach`, `branch.create|checkout`, `checkpoint.create`, `commit.create`, `rollback.plan|apply`, `replay.plan|apply`, `merge.plan|apply`, `task.update`, `message.post`, and `artifact.register`. Every command accepts `request_id`, `expected_head` where relevant, and a dry-run flag for destructive actions.

`commit.create` records a semantic version node from a snapshot; `checkpoint.create` records a resumable execution state and may be unnamed; `rollback.apply` restores selected dimensions (filesystem, agent state, or both) into a new operation. External side effects are never silently undone.

## Result contract

Successful responses return `request_id`, `status`, `result`, `warnings`, and `observed_sequence`. Long operations return a job ID and can be polled with `command.inspect` or followed through events. A command that may have executed before a transport failure is queried by the same `request_id`; callers must not blindly submit a new request.

## Agent messages

`message.post` stores a bounded, auditable collaboration event with sender, recipients or topic, related task/commit/operation IDs, classification, and retention policy. Messages are context, not a guaranteed queue: delivery, ordering, and acknowledgement are separate fields. Frameworks may provide richer messaging while projecting durable references into Pong.

## Permissions

The token grants capabilities such as `read:workspace`, `write:workspace`, `create:commit`, `apply:rollback`, `replay:operation`, `post:message`, and `read:secrets` (the last is denied by default). Authorization is evaluated on every command against actor, workspace, branch, resource sensitivity, and approval requirements.

## Concurrency

Mutations include an expected version (head, lease epoch, or resource revision). A stale expectation returns `STALE_HEAD` or `CONFLICT` with the current value and a diff cursor. Clients re-read and intentionally merge; Pong never auto-overwrites another writer's changes.

## Agent-friendly behavior

Responses are deterministic, machine-readable, and self-describing. Human-readable summaries are optional. Pagination is cursor-based, timestamps are RFC 3339, sizes are bytes, and paths are normalized relative paths within the workspace. The protocol exposes enough state for an agent to answer who/where/what-next without inspecting private storage internals.

