# ADR-M4-018: Shared Observation Within One Trusted Core

**Date:** 2026-09-24
**Status:** Accepted for External Agent Protocol v1.0's single trusted
collaboration domain; no multi-tenant authorization claim.

## Context

M4-017 found that registered Agent B can inspect Agent A's Workspace by ID.
The response includes logical head, Version Head, state, revision and active
lease authority. Version/Checkpoint/Handoff reads and diff are similarly
broad; diff exposes relative file names and hashes, though not content.
Execution and Operation reads are Agent-scoped. There is no durable Workspace
creator/owner, project membership or per-resource grant. A registered Agent
can create a Task and a compatible Execution bound to a Workspace, so using
such an Execution as a new read ACL would not establish ownership.

## Decision

Freeze a **single trusted collaboration domain** per Core. Registered Agents
may observe shared Task/Workspace/Version/Checkpoint/Handoff metadata and
diff by ID, including the detailed fields above. This is not a merely
discoverable Workspace identity. Execution/Operation visibility and control
remain owned by the Agent; mutation uses the existing Execution binding,
lease, revision, lifecycle and idempotency checks. Checkpoint Resume may
create a new Execution for a different registered Agent under the existing
Task/Version relation. Rollback and arbitrary Restore remain internal, not
new external recovery grants. The precise matrix is in
`docs/architecture/M4_RESOURCE_AUTHORIZATION.md`.

`hello` describes globally supported Protocol v1.0 commands, **not** the
current Principal's resource grants. The HTTP credential issuer must admit
only Agents trusted to see all shared metadata in this Core. JSONL relies on
its local OS caller boundary. Neither transport creates a new project/tenant
authority.

## Alternatives Considered

- Restrict `get_workspace` using existing Execution binding: rejected as
  misleading security. An Agent can self-create a compatible Execution and
  the Workspace has no durable owner; it would also disrupt cross-Agent
  continuation without providing true isolation.
- Use an active lease or expected revision as a read permission: rejected.
  They coordinate mutation and freshness, not identity or membership.
- Add project membership, per-Workspace owner, RBAC/ACL storage or Protocol
  v2: outside M4-018; this needs a separate authorized schema/policy design.
- Redact only the foreign response in the current DTO: no trustworthy
  creator/owner exists to decide who is foreign to a Workspace, and an
  incomplete DTO would silently change the v1.0 contract.

## Consequences

Cross-Agent metadata reads, including detailed Workspace state and relative
diff paths, are intentional within one trusted domain. They are **not**
multi-tenant safe. Physical locator, driver, snapshot contents and CAS blobs
are absent from the wire DTO; client-authored opaque text can still contain
arbitrary text. Explicit tests lock in shared reads, scoped Execution/
Operation state, mutation rejection without side effects, legitimate
handoff/resume, and independent-process HTTP/JSONL equivalence. No Protocol
DTO, Core business schema, HTTP adapter or production code changes.

If future deployment requires independent tenants or private per-Agent
Workspace observation, this decision does not authorize it. A new explicit
durable grant/ownership model and capability contract must be reviewed first.
MCP and SDK remain deferred.
