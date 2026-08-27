# Pong Documentation

Pong is Git-like version control and coordination infrastructure for AI-agent execution. It records and versions workspaces, environments, operations, state, artifacts, and collaboration context. It does not reason, plan, schedule, assign roles, choose models, or orchestrate agents.

This directory is the design authority for Pong v0.x. The implementation must follow the sequence **research -> design -> review -> ADR -> implementation**. Read [`roadmap/NEXT_TASK.md`](roadmap/NEXT_TASK.md) before starting work in a new session.

## Reading order

1. [`vision/PRODUCT_VISION.md`](vision/PRODUCT_VISION.md) and [`vision/DESIGN_PRINCIPLES.md`](vision/DESIGN_PRINCIPLES.md)
2. [`architecture/SYSTEM_ARCHITECTURE.md`](architecture/SYSTEM_ARCHITECTURE.md) and [`architecture/DATA_MODEL.md`](architecture/DATA_MODEL.md)
3. [`architecture/WORKSPACE_MODEL.md`](architecture/WORKSPACE_MODEL.md), [`architecture/EXECUTION_MODEL.md`](architecture/EXECUTION_MODEL.md), and [`architecture/RUNTIME_ARCHITECTURE.md`](architecture/RUNTIME_ARCHITECTURE.md)
4. [`protocol/PONG_PROTOCOL.md`](protocol/PONG_PROTOCOL.md) and [`protocol/VERSIONING_PROTOCOL.md`](protocol/VERSIONING_PROTOCOL.md)
5. [`security/SECURITY_MODEL.md`](security/SECURITY_MODEL.md) and [`reliability/RECOVERY.md`](reliability/RECOVERY.md)
6. [`architecture/ARCHITECTURE_REVIEW.md`](architecture/ARCHITECTURE_REVIEW.md) and [`architecture/DOCUMENTATION_REVIEW.md`](architecture/DOCUMENTATION_REVIEW.md)
7. ADRs and [`roadmap/ROADMAP.md`](roadmap/ROADMAP.md)

## Document map

| Area | Responsibility |
| --- | --- |
| `vision/` | Product boundary, problem, and principles |
| `architecture/` | Components, object model, runtime, storage, workspaces, execution, and events |
| `protocol/` | Stable wire, API, SDK, and CLI contracts |
| `concepts/` | Agent-facing explanations of individual domain objects |
| `decisions/ADR/` | Durable decisions and rejected alternatives |
| `security/` | Trust boundaries, permissions, and secret handling |
| `reliability/` | Failure, consistency, recovery, and idempotency |
| `compatibility/` | Git and framework integration boundaries |
| `development/` | Contribution, testing, and implementation rules |
| `roadmap/` | Milestones, next task, and change history |
| `research/` | Comparative evidence and unresolved questions |
| `governance/` | Project constitution and decision discipline |

## Status vocabulary

- **Normative**: an implementation contract; changes require an ADR when behavior or compatibility changes.
- **Proposed**: designed for a later milestone, not yet implemented.
- **Experimental**: a PoC may test the idea, but it is not production code or a supported API.

Phase 0 was documentation-only. The roadmap has now moved through an internal
M1 durable-primitives evidence cycle and bounded M2 workspace/snapshot and M3
operation-ledger slices. M1 is still not a passed release gate:
supported-platform and old-binary matrices, accepted budgets, and an unresolved
Windows fault disposition remain. The M2/M3 slices under `src/` are test-gated
development code, not a released Core, provider, or runtime API. No public CLI,
runtime interception, SDK, server, or UI is implemented.
