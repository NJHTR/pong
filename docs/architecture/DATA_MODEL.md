# Data Model

**Implementation status:** The model below is the v0.x target. The current
internal M2/M3 bounded slices persist workspace, lease, and environment records,
filesystem snapshot manifests/blobs, and the minimal operation ledger. Branches,
commits, checkpoints, task/agent registries, and provider capture remain later
milestones. The acceptance boundaries are
[`M2_WORKSPACE_SNAPSHOT_GATE.md`](../development/M2_WORKSPACE_SNAPSHOT_GATE.md)
and [`M3_OPERATION_LEDGER_GATE.md`](../development/M3_OPERATION_LEDGER_GATE.md).

## Identity and relationships

The project-scoped aggregate view is:

```text
Project
  |
  +-- Agent --------------------+
  |                              |
  +-- Workspace -----------------+-- membership / write lease
  |       |                      |
  |       +-- Environment        +-- current Branch --> Commit --> Snapshot
  |       |                                               |
  |       +-- materialization                           Operation --> Artifact
  |
  +-- Task --> Operation --> Event
  |
  +-- Event (project stream; causality links cross aggregates)
```

The execution and version graphs are related but not collapsed:

```text
Task --> Checkpoint --> Snapshot
  |         |
  +--> Agent session --> Operation --> Event

Branch --> Commit --> Snapshot
              |
              +--> operation range / task provenance
```

```text
Project 1--N Agent
Project 1--N Workspace
Project 1--N Task
Project 1--N Branch
Project 1--N Event
Workspace N--1 Environment (current binding)
Workspace N--1 Branch (current checkout)
Agent N--N Workspace (membership; one write lease at a time)
Task 1--N Operation
Operation 1--N Event
Operation N--N Artifact (input/output links)
Commit 1--1 Snapshot manifest (tree state)
Checkpoint references Snapshot + execution cursor
```

## Entity definitions

- **Project**: stable namespace, policy set, store configuration, and root refs.
- **Agent**: durable identity, framework type, capabilities, lifecycle, and current context.
- **Workspace**: logical mutable materialization with provider, lease, current branch, head, and reconciliation status.
- **Environment**: versioned description of OS/runtime/image/network/mounts plus redacted variables. The current slice records only a safe, allowlisted `described` subset; it does not claim reproducibility.
- **Branch**: mutable named ref to a commit; project-scoped, not physically tied to one workspace.
- **Commit**: immutable semantic version node with parents, tree manifest, author agent, message, and provenance range.
- **Snapshot**: immutable state capture, possibly unnamed and transient, addressed by manifest hash.
- **Checkpoint**: resumable recovery record combining a snapshot with execution cursor, task state, and resume policy.
- **Operation**: one attempted tool or lifecycle action with input/output evidence and side-effect classification.
- **Artifact**: immutable output or external reference with checksum, media type, provenance, and retention policy.
- **Event**: append-only fact about an operation or lifecycle transition; event order is explicit.
- **Task**: unit of agent work with owner, parent task, status, and desired outcome.
- **ToolCall**: normalized invocation envelope; an operation may wrap one tool call and provider-specific details.

## Cardinality decisions

An agent may have many workspaces and a workspace may have many members, but only one active write lease by default. A branch can be checked out by many workspaces. Environment records are versioned and can be reused; a workspace records the exact environment revision used for a run.

## Snapshot, checkpoint, commit

Snapshot is a factual capture. Commit is a durable, reviewable version node with parent refs and intent. Checkpoint is a recovery contract: it identifies which snapshot and execution cursor can restart a task, plus what must not be replayed automatically. A checkpoint need not create a branch commit.

## Artifact ingress

Artifacts enter history through an explicit link operation or a commit manifest. Large bytes remain in the artifact store; commits reference their digest and provenance. Untracked temporary files are not artifacts merely because they exist in a workspace.
