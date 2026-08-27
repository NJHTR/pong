# Agent Framework Integration

## Adapter boundary

Framework adapters translate lifecycle and state into Pong's stable protocol. They do not move planning, scheduling, prompts, model choice, or framework business logic into Pong Core. Each adapter is optional and versioned independently.

## Adapter interface

An adapter should provide:

- `identify_run`: map framework run/node/task IDs to Pong IDs.
- `on_tool_start|on_tool_end`: attach structured operation context.
- `on_state_checkpoint`: publish resumable state with schema/version.
- `on_message`: project collaboration references.
- `on_run_end`: flush and reconcile.
- `capabilities`: declare which hooks are native, best-effort, or unavailable.

Adapters consume Pong queries for workspace/branch/head and use standard commands for commits, checkpoints, and messages. They must propagate request IDs and never invent success when a hook is missed.

## Framework-specific notes

LangGraph node/checkpoint IDs map naturally to task and checkpoint references. AutoGen and CrewAI messages map to `message.post` plus operation links; their internal scheduling remains outside Pong. OpenHands-style workspace/tool events use runtime interception with framework metadata when available. Generic adapters may provide only run boundaries and reconciliation.

## State and serialization

Framework state is stored as a versioned, redacted artifact. Non-serializable handles, live sockets, secrets, and provider sessions are represented by opaque references and recovery requirements. An adapter declares whether a checkpoint is resumable, inspectable only, or advisory.

## Failure isolation

Adapter crashes must not corrupt core history. Pong records adapter health and missed hooks, then falls back to runtime observation/reconciliation. Adapter upgrades require compatibility tests against protocol fixtures.

