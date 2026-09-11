# M4 Provider-Neutral Control Layer

**Status:** `IMPLEMENTED` / `INTERNAL` / `TEST-GATED`
**Slice:** M4 AgentControl hardening
**Scope:** local Rust composition facade over the durable Pong core

## Purpose

The control layer gives a local caller one provider-neutral entry point for
the durable lifecycle already implemented by Pong. It is intentionally a
composition facade, not an orchestration runtime and not a provider adapter.

`AgentControl` accepts explicit requests and delegates to `MetadataStore` and
`WorkspaceManager`. SQLite transactions, leases, revision checks, CAS
validation, redaction, and existing state-transition rules remain authoritative
in those lower layers.

## Durable Workflow

The supported local workflow is:

```text
Agent -> Task -> Execution -> Workspace/Lease -> Snapshot -> Version
       -> Checkpoint -> Handoff -> Resume -> target Workspace
       -> source materialization -> target Snapshot -> Version
```

The facade exposes registration and lookup for Agents and Tasks, creation and
state transitions for Executions, Workspace creation and lease operations,
Version publication, Checkpoint/Handoff/Resume records, source Version
materialization, restore, read-only diff, rollback, and an aggregated state
view.

## Provider Boundary

Provider names such as `codex` and `claude-code` are metadata on an immutable
Agent identity. Core behavior never branches on those strings. A provider
process owns its execution environment; Pong owns durable IDs, ownership,
leases, revisions, provenance, and recovery records.

The real local validation invokes the installed `codex` and `claude` commands
against temporary directories. This proves process-boundary compatibility only.
It does not claim an SDK, MCP transport, network service, authentication
policy, or production adapter.

## Ownership and Isolation

- `Version.workspace_id` remains workspace-local and immutable.
- An Execution has explicit `workspace_id`, `base_version_id`, and
  `current_version_id` fields; references are never inferred from heads.
- A cross-workspace source Version is read-only. Materialization and restore
  publish a target-local Snapshot and update only the target Workspace head.
- `Version.parent` remains same-workspace lineage. Handoff and resume are
  separate durable relations.
- Leases and revision CAS are checked for every write. Independent Workspaces
  can proceed concurrently; shared-writer conflicts remain deterministic.

## State Inspection

`AgentControl::state` joins the explicit Execution, Task, Agent, Workspace,
lease, base/current Version, Checkpoints, Handoffs, and Resume records. The
result is a read model for local adapters and diagnostics; it is not a second
source of truth. `AgentControl::operation` exposes the existing durable
Operation record for failure and replay inspection without exposing a SQLite
connection.

## Publication Authority

`PublishVersionRequest.expected_workspace_revision` is the caller's observed
write authority. A fresh publication validates it before scanning or
publishing a Snapshot, and the same value is passed to Snapshot publication's
revision compare-and-swap. A stale caller cannot advance `Workspace.head`,
create a Version Operation, create a Version, or update Version Head.

The `version.create` Operation stores the initial Workspace revision,
Workspace head, Version Head, requested Version Head policy, and Snapshot
input. This is existing Operation persistence, not a new journal. Recovery
accepts only these durable stages:

1. the target-local Snapshot is current and the Workspace revision/Version
   Head equal the recorded post-Snapshot state; or
2. the Version exists and, when requested, Version Head equals that Version
   with exactly one additional revision transition.

Missing or foreign Snapshot metadata, changed Workspace head, unrelated
revision advancement, changed Version Head, and a changed retry envelope fail
closed. Version creation and Operation completion remain one SQLite
transaction in `MetadataStore::create_version`; an exact retry returns the
same Version and does not repeat either revision transition.

Filesystem/CAS publication and SQLite are not represented as one ACID
transaction. Snapshot recovery remains owned by the existing Workspace and
Snapshot operations.

## Evidence Boundary

The provider-neutral facade is covered by ten active contract-level cases in
`tests/control_layer.rs`, including stale revision rejection, lease and input
failures before mutation, pre-commit retry, post-commit cold-reopen recovery,
corrupt replay dependencies, concurrent mutation rejection, and three
provider-metadata values operating on isolated Workspaces. The real
Codex to Claude Code workflow is covered by the ignored,
environment-dependent `tests/agent_handoff_e2e.rs` and its local development
evidence under `artifacts/m3-development/`. Ignored contract placeholders are
not counted as passing implementation tests.

No public service contract, CLI, SDK, MCP endpoint, provider registry,
scheduler, worker pool, merge, branch, or rebase behavior is included.
`PongError` is suitable for the local Rust boundary through stable `code()`
values, but a future external protocol must define safe wire payloads rather
than serialize raw SQLite or I/O details.
