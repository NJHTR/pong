# System Architecture

## Context

Pong sits below an agent framework and beside the tools used by an agent.

```text
Agent framework
    | explicit API and adapter hooks
Agent runtime
    | tool calls, processes, filesystem access
Pong runtime
    | operation capture and context propagation
Pong core
    | identities, refs, invariants, policy
Pong stores
    | metadata, events, CAS objects, artifacts
Workspace providers
    | local path, container, remote host, sandbox
```

## Layers

### Core domain

Defines Project, Agent, Workspace, Environment, Branch, Commit, Snapshot, Checkpoint, Operation, Artifact, Event, Task, and ToolCall. It validates transitions and produces deterministic identifiers.

### Runtime

Attaches context to a tool invocation, captures before/after evidence, writes operation and event records, and reports capture confidence. It does not choose tools or schedule agents.

### Storage

Persists metadata and refs in a transactional metadata store, append-only events in an event store, immutable content in a content-addressed object store, and large artifacts in a blob store.

### Integration

Generic wrappers cover filesystem, process, and explicit API calls. Framework adapters provide lifecycle and state hooks that generic interception cannot observe.

### Interfaces

CLI, SDK, local protocol, and future service APIs project the same domain model. Interfaces must not create alternate semantics.

## Trust and process boundaries

The runtime may run in the same process as the agent, as a sidecar, or as a host service. A process boundary improves isolation but cannot guarantee capture of privileged or unmanaged actions. Pong is an audit and recovery component in v0.x, not a complete security sandbox.

## Invariants

1. Immutable objects are addressed by content hash.
2. Branch refs move atomically and identify their expected old value.
3. Every durable operation has at least one event.
4. Commit parents and checkpoint bases are immutable.
5. A workspace points to one current branch ref and one materialized state at a time.
6. Secret-bearing fields are redacted before persistence.

## Deployment decision

v0.x uses a local embedded deployment: one project directory, one Pong control process or library, and optional isolated workspace providers. The data model is service-neutral so a remote coordinator can be added without changing identities or history semantics.
