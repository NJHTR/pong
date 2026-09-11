# Pong

> Pong is an agent-first, versioned workspace and execution infrastructure for long-running and multi-agent software development.

Pong 不是给 AI 套一层 Git，而是为 AI Agent 重新设计版本、状态、任务和执行管理。项目当前是 **Internal / Experimental / Test-Gated** Rust core：M1 已发布，M2 状态引擎与 M3 Agent Execution 核心工作流已完成内部实现，M4 正在定义 provider-neutral 外部控制边界。

**Current release:** `v0.1.0`

**M1:** Released

**M2:** Internal development complete (`M2-SLICE-001` ~ `012`, internal/test-gated)

**M3:** Agent Execution core implemented (internal/test-gated)

**M4:** Provider-neutral `AgentControl` facade implemented and publication-hardened (internal/test-gated)

## About

Pong is local-first version control and execution infrastructure for AI agents. It complements Git by making agent workspaces, state, and handoffs traceable, recoverable, and safe to resume.

## The Problem

传统 Git 默认围绕人类开发者：`commit`、`branch`、`merge`、`checkout`、`rebase`。Agent 的执行模型更复杂：

- 一个 Agent 可以启动多个 SubAgent；
- 多个 Agent 可以并行开发；
- Agent 可能失败、崩溃、超时或耗尽额度；
- 任务可能从 Cursor、Codex、Claude 或其他 runtime 接管；
- 一次任务可能产生许多自动修改和多个候选状态；
- 用户需要恢复到任务开始或某个稳定状态，而不是删除历史。

因此问题不是“AI 会不会用 Git？”，而是“Git 的状态模型是否适合 Agent 的执行模型？” Pong 以显式身份、不可变状态、操作耐久性和恢复边界回答这个问题。

## Pong and Git

| Git | Pong |
| --- | --- |
| 源代码 commit、branch、merge | Agent workspace、snapshot、version、operation |
| 面向人类协作的历史 | 面向自动执行的 durable state |
| 代码树的版本关系 | 执行上下文、状态引用和恢复证据 |

两者可以同时存在：项目源代码继续使用 Git，Agent 执行状态由 Pong 记录。Pong 当前不是 Git replacement。

## Current Architecture

已实现的核心关系如下。`Workspace.head` 仍然是 Snapshot root digest；Version Head 是独立的逻辑选择，不改变 M1 Snapshot-head 语义。

```text
Workspace
    |
    +-- Snapshot Head
    |
    +-- Version Head
          |
          +-- Version
                |
                +-- Parent Version
                |
                +-- Snapshot
                      |
                      +-- CAS / Tree

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

当前实现边界：

- **Storage Core:** SQLite metadata、content-addressed storage (CAS)、filesystem、canonical identity and schema checks。
- **State Engine:** Workspace lifecycle、Snapshot、Restore、Diff、Reconciliation、Version、Version Graph、Version Head、durable Operation ledger、lease/revision guards and recovery boundaries。
- **Execution Engine:** Agent、Task、Execution、独立 Workspace、Checkpoint、Handoff、Resume、跨 Workspace materialization/diff/restore/rollback，以及显式 provenance。
- **Control Layer:** 本地 Rust `AgentControl` facade，组合 durable workflow，并执行 lease、revision CAS、ownership、replay 和 recovery 检查。
- **Agent State Layer:** Memory、Skill、Automation、Policy、Plugin、Permission、Candidate、Approval 等属于后续范围。

## Agent Execution Model

M3 已在 Core 内实现 Agent、Task、Execution、Checkpoint、Handoff、Resume 和 Rollback 的 durable records，并实现跨 Workspace 的 source validation、target-local materialization、diff、restore 与 history-preserving rollback。相关设计见 [`M3_AGENT_EXECUTION.md`](docs/architecture/M3_AGENT_EXECUTION.md)、[`M3_HANDOFF_CHECKPOINT_ROLLBACK.md`](docs/architecture/M3_HANDOFF_CHECKPOINT_ROLLBACK.md) 和 [`ADR-M3-001`](docs/decisions/ADR-M3-001-agent-execution-model.md)。

当前模型区分：

- **Agent Identity**：跨进程的 durable actor identity；provider metadata 与 credentials 分离，秘密不进入 Core。
- **Task**：工作的持久身份和协调状态。
- **Execution**：某个 Agent 为某个 Task 运行的一次具体尝试，有独立状态、Workspace、base/current Version 和 Operation 引用。
- **SubAgent / Execution Graph**：父子 Execution 是独立的、有限深度且无环的关系；它不是 Version parent，也不是 Handoff。

### Multi-Agent Workspaces

一个 Task 可以关联多个独立 Execution：

```text
Task
|
+-- Codex Execution
|     |
|     +-- Backend SubAgent
|     +-- Test SubAgent
|
+-- Cursor Execution
|
+-- Claude Review Execution
```

多个 Execution 可以从相同的 base Version 开始，并绑定独立 writable Workspace。Version parent 始终保持同 Workspace lineage；跨 Workspace 延续通过显式 Checkpoint、Handoff、Resume 和 source Version 完成。写入继续受 lease 与 revision CAS 保护。自动调度、merge/rebase 和共享可写 Workspace 策略仍不在 Core 范围内。

### Handoff

```text
Codex
  |
Task T100
  |
quota exhausted
  |
Cursor
  |
继续 Task T100
```

Handoff 保持 Task identity、Version context 和 Operation history，通过显式 context reference 把工作交给另一个 Execution。Core 已支持 durable Handoff/Resume records；本地开发证据验证了 Codex 到 Claude Code 的进程级接力，但这不等同于公开 provider adapter、SDK 或远程协议。

### Checkpoint, Rollback and Resume

```text
Task baseline
     |
Checkpoint
     |
Agent execution
     |
many versions
     |
Rollback / Resume
```

Rollback 恢复到稳定的 Version 或 Checkpoint，并保留历史 Version、Operation 和 Execution。跨 Workspace rollback 会发布 target-local Snapshot、更新目标 Workspace head、保留目标 Version Head，并保持源 Workspace 不变。

## Progress

### Completed

- M1 durable primitives release：`v0.1.0`。
- M2 core state engine：Workspace/snapshot、lifecycle、operation ledger、Version persistence、Version graph、Version Head，以及对应迁移、故障和恢复测试。
- M3 Agent Execution core：Agent、Task、Execution、Checkpoint、Handoff、Resume、跨 Workspace materialization/diff/restore/rollback，以及本地真实 Agent handoff 验证。
- M4 local control facade：provider-neutral `AgentControl` 与 typed requests/views；Version publication 支持 stale-revision rejection、durable exact retry、pre-commit failure 和 post-commit cold-reopen recovery。

### In Progress

- M4 external Agent protocol contract：定义 transport-neutral request/response、reconnect、observability、authentication ownership 与 safe wire error semantics。当前尚未选择 CLI、HTTP、MCP 或 SDK transport。

### Planned

- External provider adapters and transport
- Scheduling and orchestration
- Merge/rebase and reconciliation policy across executions
- Candidate and Approval
- Agent State layer
- Safe self-evolution

## Running Locally

### Requirements

- Rust `1.78` or a compatible newer toolchain
- A local filesystem supported by the host platform

### Build and Test

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --all --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
git diff --check
```

The repository currently has no public `pong init`, `pong checkpoint`, or `pong rollback` CLI. Read [`docs/development/DEVELOPMENT_GUIDE.md`](docs/development/DEVELOPMENT_GUIDE.md) and [`docs/roadmap/NEXT_TASK.md`](docs/roadmap/NEXT_TASK.md) before changing persistence semantics.

## Technical Principles

- immutable state
- explicit identity
- deterministic behavior
- fail closed
- crash recovery
- durable operations
- explicit version references
- no phantom success
- provider-neutral design
- agent-neutral execution model

## Explicit Limits

Pong is not production-ready, enterprise-ready, fully autonomous, or a Git replacement. There is no supported provider integration, public SDK/CLI/MCP endpoint, server mode, remote replication service, scheduler, merge/rebase policy, shared-writable-workspace policy, Candidate/Approval flow, or Agent State implementation. External side effects and credentials remain outside Core behind explicit adapter and permission boundaries.

## Documentation

- [`docs/architecture/`](docs/architecture/) — system and data contracts
- [`docs/decisions/`](docs/decisions/) — ADRs and accepted boundaries
- [`docs/development/`](docs/development/) — development and test gates
- [`docs/roadmap/`](docs/roadmap/) — milestone status and next task
- [`artifacts/`](artifacts/) — retained internal validation evidence

## Next Step

Define the M4 external Agent protocol contract over the hardened local `AgentControl` semantic boundary.

Transport-neutral request/response, reconnect, observability, authentication ownership, and safe wire errors must be specified before selecting CLI, HTTP, MCP, SDK, or provider-specific adapters. See [`NEXT_TASK.md`](docs/roadmap/NEXT_TASK.md).
