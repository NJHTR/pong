# Components

| Component | Responsibility | Explicit non-responsibility |
| --- | --- | --- |
| Core domain | Objects, invariants, transitions, ids | Agent reasoning or scheduling |
| Context manager | Propagates agent, task, workspace, and operation context | Inferring business intent |
| Runtime recorder | Captures tool calls and state evidence | Guaranteeing observation of unmanaged privileged actions |
| Policy engine | Redaction, permissions, replay and rollback decisions | Replacing host security controls |
| Workspace manager | Creates, leases, materializes, and reconciles logical workspaces | Owning a specific container platform |
| Version manager | Branches, commits, diffs, merges, refs | Resolving semantic application conflicts automatically |
| Execution graph | Causal links between operations, tasks, agents, and checkpoints | Scheduling execution |
| Agent registry | Identity, lifecycle, current context, heartbeat | Agent role assignment |
| Metadata store | Transactional relational records and refs | Large binary payloads |
| Event store | Append-only operation and lifecycle events | Mutable projections as source of truth |
| Object store | Immutable CAS snapshots and manifests | Secret storage |
| Artifact store | Large outputs with checksums and retention metadata | Arbitrary untracked files |
| Integration adapters | Framework lifecycle and state hooks | Framework orchestration |
| CLI/SDK/API | Human and agent access to core operations | Separate domain rules |

## Component contract

Components communicate through versioned domain commands and events. A component may add metadata, but may not mutate an immutable record or bypass policy checks. Failure responses include whether the requested action was applied, pending reconciliation, or definitely rejected.

## Extension points

Workspace providers, event sinks, object stores, framework adapters, and policy providers are replaceable interfaces. v0.x ships local implementations only; extension points are documented before implementation.
