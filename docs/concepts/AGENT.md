# Agent

**Status: Normative.** An Agent is a first-class identity that performs or causes operations. Pong records identity and authorization; it does not define the agent's reasoning or role.

An agent record contains `agent_id`, project membership, declared name and capabilities, framework/runtime adapter, lifecycle state (`registered`, `active`, `paused`, `crashed`, `terminated`), current task, workspace leases, current environment, last operation, last checkpoint, and audit timestamps. Capability declarations are claims checked against the permission model, not authority by themselves.

An agent may own zero or many workspaces and may have at most one mutable lease per workspace. The registry is eventually queryable from events but critical writes use the metadata transaction. Agent identity is stable across process restarts when a credential-backed registration is reused; a new process incarnation receives a separate `session_id`.

Agents discover context through the explicit API: who they are, project, workspace, branch, head, peers, task, and recoverable checkpoints. Framework adapters translate native run IDs into Pong sessions without leaking framework-specific concepts into Core.

