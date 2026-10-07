# Pong

<p align="center">
  <strong>English</strong> |
  <a href="./README.zh-CN.md">简体中文</a> |
  <a href="./README.ja.md">日本語</a> |
  <a href="./README.ru.md">Русский</a>
</p>

<p align="center">
  <a href="https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml"><img alt="CI" src="https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml/badge.svg?branch=dev"></a>
  <a href="https://github.com/NJHTR/pong/releases/tag/v0.1.0"><img alt="Release v0.1.0" src="https://img.shields.io/badge/release-v0.1.0-2ea44f"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.78+" src="https://img.shields.io/badge/rust-1.78%2B-000000?logo=rust"></a>
  <a href="https://www.apache.org/licenses/LICENSE-2.0"><img alt="License Apache 2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <a href="./docs/roadmap/ROADMAP.md"><img alt="Status Experimental" src="https://img.shields.io/badge/status-experimental-orange"></a>
</p>

> Agent-first, versioned workspace and execution infrastructure for long-running and multi-agent software development.

Pong is not a Git wrapper for AI. It provides a durable model for agent versions, state, tasks, workspaces, and execution. The project is currently an **internal, experimental, test-gated** Rust core.

## Project Status

| Milestone | Status |
| --- | --- |
| Current release | `v0.1.0` |
| M1 durable primitives | Released |
| M2 state engine | Internal implementation complete |
| M3 agent execution core | Internal implementation complete |
| M4 provider-neutral `AgentControl` | Implemented and publication-hardened; external protocol contract in progress |

## About

Pong is local-first version control and execution infrastructure for AI agents. It complements Git by making agent workspaces, state transitions, checkpoints, handoffs, and recovery traceable and durable.

Git manages source history. Pong manages the state and evidence created while agents execute work. Pong is a Rust library/core, not a model runtime, scheduler, public CLI, server, SDK, or UI.

## Why Pong?

Agent execution has requirements that do not map cleanly to a human-centered Git workflow:

- one agent can start multiple subagents;
- multiple executions can work in parallel;
- an execution can fail, crash, time out, or exhaust its quota;
- a task can move between Codex, Claude, Cursor, or another runtime;
- one task can produce many intermediate and candidate states;
- recovery must return to a known task state without deleting history.

Pong addresses these requirements with explicit identity, immutable state, durable operations, deterministic recovery, and auditable ownership boundaries.

## Pong and Git

| Git | Pong |
| --- | --- |
| Commits, branches, merges | Agent workspaces, snapshots, versions, operations |
| Human collaboration history | Durable automated-execution state |
| Source-tree lineage | Execution context, state references, recovery evidence |

They are designed to coexist. Pong is not a Git replacement.

## Architecture

`Workspace.head` is the Snapshot root digest. Version Head is a separate logical selection and does not change Snapshot-head semantics.

```text
Workspace
    |
    +-- Snapshot Head
    |
    +-- Version Head -- Version -- Parent Version
                              |
                              +-- Snapshot -- CAS / Tree

Task
    |
    +-- Execution -- Workspace / Lease
          |
          +-- Checkpoint / Handoff / Resume

AgentControl
    |
    +-- MetadataStore + WorkspaceManager
          |
          +-- Operation / Snapshot / Version / Rollback
```

Implemented layers:

- **Storage core:** SQLite metadata, content-addressed storage, filesystem publication, canonical identity, and schema validation.
- **State engine:** Workspace lifecycle, Snapshot, restore, diff, reconciliation, Version graph and Head, durable Operation ledger, leases, revision guards, and recovery boundaries.
- **Execution engine:** Agent, Task, Execution, independent Workspaces, Checkpoint, Handoff, Resume, cross-workspace materialization/diff/restore/rollback, and explicit provenance.
- **Control layer:** local Rust `AgentControl` facade with typed requests and views over the durable workflow.

Memory, Skill, Automation, Policy, Plugin, Permission, Candidate, and Approval belong to later milestones.

## Agent Execution

Each Agent has a durable identity. A Task identifies the work, while an Execution represents one concrete attempt by one Agent. An Execution has explicit Workspace, base Version, current Version, and Operation references.

Parent and child Executions form a bounded, acyclic execution graph. This relation is separate from Version ancestry and from Handoff.

### Multi-Agent Workspaces

```text
Task
|
+-- Codex Execution
|     +-- Backend SubAgent
|     +-- Test SubAgent
|
+-- Cursor Execution
+-- Claude Review Execution
```

Executions can start from the same base Version while using independent writable Workspaces. Version parents always remain within one Workspace. Cross-workspace continuation uses explicit Checkpoint, Handoff, Resume, and source Version records. Every write remains protected by leases and revision compare-and-swap.

### Handoff and Recovery

Pong persists Handoff and Resume records without rewriting Task identity or fabricating Version ancestry. Local development evidence validates a process-level handoff from Codex to Claude Code, but this is not yet a supported provider adapter or remote protocol.

Rollback preserves Version, Operation, and Execution history. Cross-workspace rollback publishes a target-local Snapshot, updates only the target Workspace head, preserves its Version Head, and leaves the source Workspace unchanged.

## Progress

### Completed

- M1 durable primitives release: `v0.1.0`.
- M2 state engine and its migration, fault, and recovery coverage.
- M3 execution records, cross-workspace workflows, and local real-agent handoff validation.
- M4 local `AgentControl`, including stale-revision rejection, exact durable retry, pre-commit failure recovery, and post-commit cold-reopen recovery.

### In Progress

- M4 external Agent protocol contract: transport-neutral requests and responses, reconnect behavior, observability, authentication ownership, and safe wire errors.

### Planned

- external provider adapters and transport;
- scheduling and orchestration;
- merge, rebase, and reconciliation policy across executions;
- Candidate and Approval;
- Agent State layer;
- safe self-evolution.

## Run Locally

Requirements:

- Rust `1.78` or a compatible newer toolchain;
- a local filesystem supported by the host platform.

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --all --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
git diff --check
```

The focused `pong` command exposes project initialization, status discovery, and
durable checkpoint recovery:

```bash
pong init
pong init <project-root>
```

Initialization is local and does not require Git. It creates Pong-owned
metadata under `.pong/` and leaves existing project files untouched. The
`pong status` command searches the current directory and its ancestors for the
nearest Pong project, or accepts an explicit project root:

```bash
pong --help
pong status
pong status --project-root <project-root> --json
pong start "describe the work to begin"
pong start "describe the work to begin" --json
pong start "import the existing project" --from-project --json
pong recovery --help
pong recovery inspect --repository <pong-repository> --checkpoint <checkpoint-id>
pong recovery resume --repository <pong-repository> --checkpoint <checkpoint-id>
```

`inspect` prints the linked Task, Execution, Workspace, Version, Snapshot, and
resume attempts. `resume` creates a new durable Execution attempt through the
existing checkpoint-resume API. These commands do not start or configure any
Agent tool; launch and selection remain user-controlled. Add `--json` for
machine-readable output.

`pong start` creates a durable Task, an isolated local Workspace outside the
project's `.pong` control directory, and a running Execution. It does not
launch Codex, Claude Code, or another provider. The returned IDs and workspace
path are the context to give to whichever Agent session you start yourself.

Use `--from-project` when the new execution should begin with a copy of the
existing project tree. Pong copies ordinary files into the isolated Workspace,
skips the project's top-level `.pong` control directory, publishes an initial
Snapshot and Version, and records that Version on the Execution. The source
project is not modified and Git is not required.

The command creates or reopens the Pong-owned `.pong` layout, writes the
existing v1 bootstrap descriptor when absent, and leaves existing user files
untouched. Read the [development guide](docs/development/DEVELOPMENT_GUIDE.md)
and [next task](docs/roadmap/NEXT_TASK.md) before changing persistence
semantics.

## Technical Principles

- immutable state;
- explicit identity and ownership;
- deterministic behavior;
- fail-closed validation;
- crash recovery and durable operations;
- explicit Version references;
- no phantom success;
- provider-neutral, agent-neutral design.

## Explicit Limits

Pong is not production-ready, enterprise-ready, fully autonomous, or a Git replacement. It does not provide a supported provider integration, public SDK, general CLI suite, MCP endpoint, server mode, remote replication, scheduler, merge/rebase policy, shared writable Workspace policy, Candidate/Approval flow, or Agent State implementation. The focused CLI is limited to local initialization, status discovery, durable checkpoint inspection, and resume; credentials and external side effects remain outside Core.

## Documentation

- [Architecture](docs/architecture/)
- [Architecture decisions](docs/decisions/)
- [Development and test gates](docs/development/)
- [Roadmap and next task](docs/roadmap/)
- [Validation evidence](artifacts/)

## Next Step

Read the project workflow and recovery documentation before extending the
focused CLI into additional bindings or Agent-specific adapters. See
[NEXT_TASK.md](docs/roadmap/NEXT_TASK.md).
