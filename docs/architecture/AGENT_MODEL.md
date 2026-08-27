# Agent Model

## Identity

An Agent is a durable project identity, not merely a process. It has `agent_id`, display name, framework and runtime descriptors, capability declarations, owner or tenant, lifecycle status, and audit metadata. Credentials are references to a secret manager, never values in the record.

## Registry

The registry answers current agent, workspace, branch, head, environment revision, task, last operation, and last checkpoint. Heartbeats are advisory. A stale heartbeat marks an agent as `unknown` or `disconnected`; it does not prove termination.

## Lifecycle

`registered -> starting -> active -> idle -> stopping -> stopped`, with `failed` and `unknown` as observable exceptional states. A new process may resume an existing identity only with an ownership token or explicit takeover policy.

## Collaboration

Agents share context through commits, branches, artifacts, tasks, and events. Agent messages are first-class operation payloads with sender, recipient or topic, correlation id, and retention policy. Pong records messages; it does not guarantee delivery, ordering across external transports, or business-level semantics.

## Capabilities

Capabilities are declared and enforced by the integration/runtime boundary: read workspace, write workspace, spawn agent, inspect other agents, create refs, restore state, replay operation, or export artifacts. Capability changes are audited events.

## State boundaries

Pong stores execution metadata and references to framework state. It does not treat hidden model memory, prompts, or chain-of-thought as recoverable agent state. Adapters may store opaque, redacted state blobs with explicit retention and sensitivity labels.
