# Design Principles

## 1. Agent first

Agent identity, current context, and discoverability are first-class. APIs expose the same core facts to agents and humans.

## 2. Framework agnostic core

The core defines objects and invariants, not planning or orchestration. Framework-specific behavior belongs in adapters.

## 3. Explicit state, implicit capture

Runtime interception records ordinary tool use automatically. Explicit APIs provide intentional checkpoints, commits, and queries.

## 4. History before convenience

An operation is durable before it is reported as captured. Ambiguous or partially captured work is marked as such rather than silently omitted.

## 5. Separate facts from interpretation

Events record observed facts. Commits and checkpoints add user or agent intent without rewriting the event history.

## 6. Reversibility is classified, never assumed

Filesystem changes may be reversible; network and production effects generally are not. Rollback policies must state their scope and limits.

## 7. Local-first, portable later

The default store works on one host and can be copied or replicated. Remote services are optional implementations of the same contracts.

## 8. Isolation by default

One active writer per workspace is the default. Shared workspaces require an explicit lease and conflict policy.

## 9. Deterministic identifiers and idempotency

Operation and event ids are stable across retries. Replaying a request must not duplicate a recorded fact.

## 10. Least privilege and redaction

Capture sees only the capabilities granted to the runtime. Secrets are redacted before persistence and never reconstructed from ordinary history.

## 11. Observable failure

Crashes, dropped capture, uncertain outcomes, and reconciliation are visible states, not hidden exceptions.

## 12. Documented evolution

Schema, protocol, and compatibility changes require a documented decision and migration strategy.
