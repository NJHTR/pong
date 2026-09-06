# Pong

> Pong is an agent-first, versioned workspace and execution infrastructure for long-running and multi-agent software development.

Pong 不是给 AI 套一层 Git，而是为 AI Agent 重新设计版本、状态、任务和执行管理。项目当前是 **Internal / Experimental / Test-Gated** Rust core：M1 已发布，M2 状态引擎已完成内部切片，M3 正在从 contract 进入实现。

**Current release:** `v0.1.0`

**M1:** Released

**M2:** Internal development complete (`M2-SLICE-001` ~ `012`, internal/test-gated)

**M3:** Agent Execution in development (`M3-SLICE-001A = CONTRACT_READY`)

## What Is Pong?

Pong 为 Agent 工作空间提供可验证、持久化的状态边界。它记录 Workspace、Snapshot、Version 和 Operation 的身份与关系，使自动化修改在崩溃、重试、恢复和审计时仍有明确的 durable state。

Pong 是本地优先的 Rust library/core，不是 Agent provider、模型运行时、调度器、公开 CLI、服务器或 UI。它与 Git 互补：Git 管理源代码历史，Pong 管理 Agent 执行过程中产生的工作空间状态和证据。

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

Operation
    |
    +-- Version
    +-- Workspace lifecycle
```

当前实现边界：

- **Storage Core:** SQLite metadata、content-addressed storage (CAS)、filesystem、canonical identity and schema checks。
- **State Engine:** Workspace lifecycle、Snapshot、Restore、Diff、Reconciliation、Version、Version Graph、Version Head、durable Operation ledger、lease/revision guards and recovery boundaries。
- **Execution Engine:** Task、Agent、Execution、SubAgent、Handoff、Checkpoint、Rollback 等正在设计和实现中。
- **Agent State Layer:** Memory、Skill、Automation、Policy、Plugin、Permission、Candidate、Approval 等属于后续范围。

## Agent Execution Model

`M3-SLICE-001A = CONTRACT_READY`。该 slice 是 proposal-only contract；它没有添加生产类型、SQLite DDL、provider、CLI、SDK、UI，也没有实现 Handoff、Checkpoint、Rollback、Candidate 或 Approval。相关设计见 [`M3_AGENT_EXECUTION.md`](docs/architecture/M3_AGENT_EXECUTION.md)、[`M3_EXECUTION_GRAPH.md`](docs/architecture/M3_EXECUTION_GRAPH.md) 和 [`ADR-M3-001`](docs/decisions/ADR-M3-001-agent-execution-model.md)。

目标模型区分：

- **Agent Identity**：跨进程的 durable actor identity；provider metadata 与 credentials 分离，秘密不进入 Core。
- **Task**：工作的持久身份和协调状态。
- **Execution**：某个 Agent 为某个 Task 运行的一次具体尝试，有独立状态、Workspace、base/current Version 和 Operation 引用。
- **SubAgent / Execution Graph**：父子 Execution 是独立的、有限深度且无环的关系；它不是 Version parent，也不是 Handoff。

### Multi-Agent Direction

目标架构中的一个任务可能这样展开：

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

多个 Execution 可以并行，并从相同的 base Version 开始。每个 writable Execution 默认绑定独立 Workspace，并继续遵守现有 lease 和 revision CAS。最终通过显式 reconciliation 汇合。这是当前发展方向，不是已经全部实现的运行时能力。

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

Handoff 的目标是保持 Task identity、Version context 和 Operation history，通过显式 context reference 把工作交给另一个 Execution。Handoff 当前仍是未来 slice，不会重写身份或伪造 Version。

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

Rollback 应恢复到稳定的 Version 或 Checkpoint，并从那里创建新的尝试；它不删除历史 Version、Operation 或 Execution。Checkpoint、Rollback 和 Resume 当前仍是未来能力。

## Progress

### Completed

- M1 durable primitives release：`v0.1.0`。
- M2 core state engine：Workspace/snapshot、lifecycle、operation ledger、Version persistence、Version graph、Version Head，以及对应迁移、故障和恢复测试。

### In Progress

- M3 Agent Execution：当前为 `M3-SLICE-001A = CONTRACT_READY`；35 个 contract-only tests 明确标记为 `NOT_IMPLEMENTED_CONTRACT_TEST` 并被 ignored，不计作 PASS 或 runtime evidence。

### Planned

- Multi-agent parallel execution
- Handoff
- Checkpoint and Rollback
- Reconciliation across executions
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

Pong is not production-ready, enterprise-ready, fully multi-agent, fully autonomous, or a Git replacement. There is no supported provider integration, public SDK/CLI, server mode, remote replication service, shared-writable-workspace policy, Candidate/Approval flow, or Agent State implementation. External side effects and credentials remain outside Core behind explicit adapter and permission boundaries.

## Documentation

- [`docs/architecture/`](docs/architecture/) — system and data contracts
- [`docs/decisions/`](docs/decisions/) — ADRs and accepted boundaries
- [`docs/development/`](docs/development/) — development and test gates
- [`docs/roadmap/`](docs/roadmap/) — milestone status and next task
- [`artifacts/`](artifacts/) — retained internal validation evidence

## Next Step

`M3-SLICE-001B — IMPLEMENT AGENT EXECUTION CORE`

This is the single next slice. M1 remains **RELEASED**, M2 remains **INTERNAL / TEST-GATED**, and M3-001A remains **CONTRACT_READY**.
