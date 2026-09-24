# ADR-M4-017: External Capability Surface

- **Status:** Accepted inventory / checkpoint blocked on read authorization
- **Date:** 2026-09-24
- **Baseline:** `0b17cc1920df6347466b573539e8ab58f4b8764f`

## Context

Core and AgentControl include recovery and filesystem operations beyond the
frozen External Agent Protocol v1.0. Exposing every public Rust method would
grant remote Agent Runtimes recovery authority without a corresponding
Principal, project, or operator authorization contract.

## Decision

Keep rollback inside the Core-owned embedded/recovery surface and defer any
external rollback capability. Keep arbitrary-destination Snapshot restore
internal. Continue exposing the existing guarded `materialize_version`
workflow, read-only Workspace/Version diff, lease acquire/renew/release,
checkpoint, handoff, resume, and Operation reconciliation. Do not add a
Protocol command, version, Core schema, authentication mode, MCP adapter, or
SDK in this decision.

Rollback is history-preserving but changes physical Workspace contents and
head authority. It needs explicit Execution binding, lease, revision, target,
reason/actor and durable request identity, plus prepared-outcome recovery. The
Core has much of this machinery, but Protocol v1.0 has neither an external
operator grant nor rollback result/query semantics. Automatic Agent exposure
would be a privilege expansion, not a transport feature.

## Authorization Finding

The current wire enforces Principal-to-Agent binding, Agent registration,
Execution/Operation ownership and lease/revision guards on writes. It does
not scope several OBSERVE queries or diff to an owned Execution, Workspace,
or project membership. This is an explicit `PARTIAL` boundary. The present
remote topology is suitable only for one trusted collaboration domain, not
untrusted multi-tenant/project read isolation. No new implicit RBAC rule or
breaking v1.0 query behavior is introduced under this checkpoint.

## Consequences

- Normal Agent continuation remains fully expressible via Protocol v1.0.
- `hello` remains an exact allowlist; unknown commands fail closed as invalid
  envelopes and unsupported versions retain their existing error.
- Core startup recovery and rollback remain separate from external
  `resolve_operation` and Checkpoint Resume.
- M4-017 records a partial gate and **does not** create the requested
  conditional checkpoint until read authority is decided and tested.
- MCP and SDK remain deferred. Linux/macOS, TLS, public Internet and production
  identity integration remain outside this evidence.
