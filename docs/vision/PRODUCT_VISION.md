# Pong Product Vision

## Positioning

Pong is Git-like version control and coordination infrastructure for AI agents. It versions and tracks execution environments, workspaces, state, actions, branches, artifacts, and collaboration context.

Pong is not a multi-agent framework. It does not perform reasoning, planning, scheduling, role assignment, prompt engineering, model selection, or business-level agent orchestration.

## User promise

After an agent framework integrates Pong, a human or agent can answer:

- Who performed an action, in which project, workspace, environment, and branch?
- What changed, which evidence supports the change, and what is the current head?
- Which states are safe to restore, and which side effects cannot be undone?
- What did other agents do, and how can their work be compared, replayed, or merged?

## Product shape

Pong combines an implicit runtime recorder with an explicit agent API. Normal tool calls are observed through adapters and wrappers; agents can query status, inspect history, create checkpoints, commit, branch, diff, rollback, and request replay through a stable API.

The source of truth is a project-scoped history containing a version DAG and an execution DAG. Version nodes describe meaningful states. Execution nodes describe observed operations and their causal relationships.

## v0.x product boundary

v0.x is local-first, single-host, framework-agnostic, and designed for deterministic recovery of filesystem and metadata state. It provides an embedded store, an explicit integration contract, and conservative side-effect classification. Distributed coordination, provider-specific browser/database capture, and automatic reversal of external effects are outside the initial product.

## Success criteria

1. A new agent can discover identity, workspace, branch, head, environment summary, and active task without framework-specific knowledge.
2. Every captured operation has an immutable event record, correlation id, result, and capture confidence.
3. A user can restore a filesystem workspace to a committed or checkpointed state without corrupting history.
4. Two agents can compare work through branches, commits, artifacts, and events without sharing a mutable directory by default.
5. Integrations can evolve without changing the core object model.

## Long-term direction

Pong may later add remote object replication, policy enforcement, multi-host execution, and framework adapters. Those extensions must preserve the local object model and explicit trust boundaries.
