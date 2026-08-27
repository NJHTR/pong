# Architecture Review

## 1. What problem does Pong solve?

It gives agent systems a durable, inspectable history of identity, workspace state, environment, operations, artifacts, and collaboration context, with controlled restore and replay semantics.

## 2. Boundary with Git

Git versions content and refs. Pong versions execution context and state transitions, then uses content-addressed snapshots and Git-like refs where useful. Pong must not pretend that external side effects are files or that a commit undoes them.

## 3. Difference from AgentGit

AgentGit-like projects can provide agent-aware commits or trajectories. Pong's intended scope is broader: provider-neutral workspaces, environment revisions, operation evidence, event history, checkpoints, policy, and integration contracts. This distinction must be validated by a focused competitive study before claiming uniqueness.

## 4. Difference from LangGraph checkpointing

LangGraph checkpointing persists graph execution state for resume. Pong records cross-tool operations, filesystem and environment evidence, version DAGs, and multi-agent context. An adapter may reference a LangGraph checkpoint; it must not duplicate or reinterpret its internal semantics.

## 5. Boundary with multi-agent frameworks

Frameworks own reasoning, planning, scheduling, roles, prompts, model providers, and business messages. Pong owns durable context, history, recoverable state, and coordination evidence.

## 6. Minimal viable core

Project and agent identity, one managed workspace, SQLite metadata and events, filesystem snapshot/commit, branch refs, operation envelopes, redaction, crash journal, and status/diff/log/restore APIs. Runtime wrappers and one framework-neutral adapter are enough to validate the model.

## 7. Potential over-design

Remote replication, Kubernetes providers, full browser/database interception, semantic merge, universal process checkpointing, and automatic rollback of external effects should not be built before real workloads justify them.

## 8. Technical risks

- Capturing unmanaged or privileged actions is inherently incomplete.
- Filesystem snapshots can be expensive and platform-specific.
- Environment metadata may leak secrets despite redaction.
- Replay cannot guarantee identical external data or model output.
- Concurrent writers can create ambiguous causality.
- SQLite locking and provider behavior may limit scale.

## 9. PoC requirements

Prove atomic filesystem snapshot and recovery after crash; measure incremental CAS cost; test operation capture around subprocesses; validate secret redaction; and run two-agent branch/merge and lease-conflict scenarios.

## 10. Unresolved questions

How much opaque framework state is safe and useful to persist? Which provider guarantees are required for production? What event retention and privacy model satisfies regulated deployments? Can deterministic replay be meaningfully defined for model and browser calls?

## 11. v0.1 should implement

Local project initialization, registry, one writable workspace, environment fingerprint, managed file/process capture, snapshots, commits, branches, diff/log/status, checkpoints, restore of local state, event export, and explicit irreversible-operation approvals.

## 12. v0.1 should not implement

Distributed consensus, automatic security sandboxing, arbitrary remote filesystem capture, provider-specific browser/database proxies, automatic external compensation, or a web UI.

## 13. Core technical moat

The moat is a trustworthy causal model joining agent identity, tool operations, workspace versions, environment revisions, and recovery policy without claiming impossible observability or reversibility.

## 14. Most likely failure mode

Pong may become a noisy event logger that users do not trust because records are incomplete, secrets leak, or rollback promises exceed provider guarantees. The product must expose uncertainty, keep capture costs bounded, and make recovery scope explicit.

## Review verdict

The architecture is coherent for a local-first Phase 0, but confidence depends on the PoCs above. Implementation should begin only after documenting their results and converting any changed assumptions into ADRs.
