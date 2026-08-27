# Problem Statement

## The problem

Agent frameworks execute long, stateful, and tool-heavy workflows. Existing version control records source files, while checkpoint systems usually record model state or graph progress. Neither gives a coherent, inspectable history of the agent, its environment, its operations, and the resulting workspace.

This creates practical failures:

- A file change is visible but its authoring tool call and environment are not.
- A checkpoint can resume a graph but cannot explain external filesystem changes.
- Multiple agents work in opaque directories and lose causal context.
- Rollback is confused with undo even when an HTTP request, email, or deployment already occurred.
- Reproducing a result requires guessing tool inputs, versions, and environment details.

## Jobs to be done

Pong must let humans and agents:

1. Observe execution context and recent activity.
2. Record state transitions with provenance.
3. Isolate and compare concurrent work.
4. Restore recoverable state after interruption or error.
5. Replay eligible operations with explicit policy and evidence.
6. Share collaboration context without becoming a message broker.

## Non-goals

Pong will not decide what an agent should think or do. It will not schedule agents, select LLMs, define roles, or own application-level messages. A framework may use Pong commits and events as shared context, but remains responsible for orchestration.

## Constraints

- Capture must tolerate agent and host crashes.
- Secrets must not enter ordinary commits, events, or snapshots.
- Operations can be partially observable and can have irreversible side effects.
- A logical workspace must not depend on a particular local path or container runtime.
- The core must remain usable without a network connection.

## Outcome

The desired outcome is a trustworthy execution history: a tamper-evident record of what was attempted, what changed, what can be restored, and what requires human approval.
