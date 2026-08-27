# Workspace

**Status: Normative.** A Workspace is Pong's logical, addressable view of an agent's project files and execution context. It is not a path and it is not a container.

The internal M2 slice currently realizes only the local-driver metadata,
epoch-lease, environment binding, tree snapshot, and new-directory
materialization subset. Full lifecycle, reconciliation, commit linkage, and
non-local providers remain gated work; see
[`M2_WORKSPACE_SNAPSHOT_GATE.md`](../development/M2_WORKSPACE_SNAPSHOT_GATE.md).

## Identity and lifecycle

`workspace_id` is immutable and globally unique within a Pong project. A workspace has a `project_id`, optional `owner_agent_id`, a `workspace_driver` (local, container, remote, or sandbox), a current branch ref, a materialized head, an environment binding, status, and redacted metadata. Creation, attach, detach, migrate, freeze, resume, and archive are events.

The driver owns physical placement. Pong stores a logical locator and driver capabilities, never assuming that `/workspace/a` or a container name is stable. Migration creates a new materialization for the same logical workspace and records continuity; it does not silently change history.

## Ownership and sharing

The default is one active agent per mutable workspace. A workspace may be shared only through an explicit lease or read-only view. Two agents must not mutate the same writable workspace concurrently in v0.1. Collaboration uses branches, commits, artifacts, and events, then an agent may materialize another branch into its own workspace.

An agent can own multiple workspaces (for separate tasks, platforms, or recovery points). A workspace has at most one active mutable branch lease at a time; branch identity itself is project-scoped rather than physically owned by a workspace.

## Consistency

The workspace head is a materialized view of a commit plus uncommitted observable operations. A write-ahead operation record precedes acknowledgement of a captured mutation. A commit advances the branch only when the workspace lease, operation sequence, and parent head still match. Stale clients receive a conflict instead of an implicit overwrite.

## What is and is not versioned

Tracked files, declared configuration, safe environment facts, operation references, and artifact manifests may be snapshotted. Secrets, transient credentials, host-specific paths, and uncontrolled external state are excluded or replaced by redacted attestations. A workspace snapshot is not proof that a database or remote service can be restored.

The current local snapshot captures regular files and directories only. It
rejects symlinks/reparse points, traversal and non-portable names, and enforces
file-count and per-file-size limits. It does not preserve file modes, ACLs,
ownership, sparse/hard links, devices, sockets, or process state.
