# Pong

<p align="center">
  <a href="./README.md">English</a> |
  <strong>简体中文</strong> |
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

> 面向长时运行与多 Agent 软件开发的 Agent-first 版本化工作空间和执行基础设施。

Pong 不是给 AI 套一层 Git，而是为 Agent 的版本、状态、任务、工作空间和执行过程提供持久化模型。项目当前是**内部、实验性、测试门禁**的 Rust Core。

## 项目状态

| 里程碑 | 状态 |
| --- | --- |
| 当前版本 | `v0.1.0` |
| M1 durable primitives | 已发布 |
| M2 状态引擎 | 内部实现完成 |
| M3 Agent Execution Core | 内部实现完成 |
| M4 provider-neutral `AgentControl` | 已实现并完成发布流程硬化；外部协议契约进行中 |

## 项目简介

Pong 是本地优先的 AI Agent 版本控制与执行基础设施。它与 Git 配合，使 Agent 工作空间、状态迁移、Checkpoint、Handoff 和恢复过程可追踪、可恢复并持久保存。

Git 管理源代码历史，Pong 管理 Agent 执行过程中产生的状态和证据。Pong 是 Rust library/core，不是模型运行时、调度器、公开 CLI、服务器、SDK 或 UI。

## 为什么需要 Pong？

Agent 执行具有传统人类中心 Git 工作流难以直接表达的需求：

- 一个 Agent 可以启动多个 SubAgent；
- 多个 Execution 可以并行工作；
- Execution 可能失败、崩溃、超时或耗尽额度；
- Task 可能在 Codex、Claude、Cursor 或其他 runtime 之间接力；
- 一个 Task 可能产生大量中间状态和候选状态；
- 恢复必须回到明确的 Task 状态，同时保留历史。

Pong 使用显式身份、不可变状态、durable Operation、确定性恢复和可审计的所有权边界解决这些问题。

## Pong 与 Git

| Git | Pong |
| --- | --- |
| Commit、branch、merge | Agent Workspace、Snapshot、Version、Operation |
| 面向人类协作的历史 | 面向自动执行的持久状态 |
| 源代码树的版本关系 | 执行上下文、状态引用和恢复证据 |

两者可以同时使用。Pong 不是 Git replacement。

## 架构

`Workspace.head` 是 Snapshot root digest。Version Head 是独立的逻辑选择，不改变 Snapshot Head 语义。

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

已实现的层次：

- **Storage Core：** SQLite metadata、content-addressed storage、文件系统发布、canonical identity 和 schema validation。
- **State Engine：** Workspace lifecycle、Snapshot、restore、diff、reconciliation、Version graph/Head、durable Operation ledger、lease、revision guard 和恢复边界。
- **Execution Engine：** Agent、Task、Execution、独立 Workspace、Checkpoint、Handoff、Resume、跨 Workspace materialization/diff/restore/rollback 和显式 provenance。
- **Control Layer：** 本地 Rust `AgentControl` facade，通过 typed request/view 组合 durable workflow。

Memory、Skill、Automation、Policy、Plugin、Permission、Candidate 和 Approval 属于后续里程碑。

## Agent Execution

每个 Agent 都有持久身份。Task 标识工作，Execution 表示一个 Agent 的一次具体执行尝试。Execution 显式记录 Workspace、base Version、current Version 和 Operation 引用。

父子 Execution 构成深度受限且无环的执行图。它与 Version ancestry 和 Handoff 是不同关系。

### 多 Agent 工作空间

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

多个 Execution 可以从相同 base Version 开始，同时使用独立的 writable Workspace。Version parent 始终属于同一个 Workspace；跨 Workspace 延续通过显式 Checkpoint、Handoff、Resume 和 source Version record 完成。所有写入都受 lease 和 revision compare-and-swap 保护。

### 接力与恢复

Pong 持久化 Handoff 和 Resume record，不会重写 Task identity 或伪造 Version ancestry。本地开发证据已验证 Codex 到 Claude Code 的进程级接力，但这还不是受支持的 provider adapter 或远程协议。

Rollback 保留 Version、Operation 和 Execution 历史。跨 Workspace rollback 会发布 target-local Snapshot，只更新目标 Workspace head，保留目标 Version Head，并保持源 Workspace 不变。

## 进度

### 已完成

- M1 durable primitives release：`v0.1.0`。
- M2 状态引擎，以及迁移、故障和恢复测试。
- M3 execution records、跨 Workspace workflow 和本地真实 Agent handoff 验证。
- M4 本地 `AgentControl`，包括 stale revision 拒绝、durable exact retry、pre-commit failure recovery 和 post-commit cold-reopen recovery。

### 进行中

- M4 外部 Agent 协议契约：transport-neutral request/response、reconnect、observability、authentication ownership 和安全的 wire error。

### 计划中

- 外部 provider adapter 与 transport；
- 调度与 orchestration；
- 跨 Execution 的 merge、rebase 和 reconciliation policy；
- Candidate 与 Approval；
- Agent State layer；
- 安全的自我演进。

## 本地运行

要求：

- Rust `1.78` 或兼容的新版本工具链；
- 当前平台支持的本地文件系统。

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --all --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
git diff --check
```

项目当前没有公开的 `pong init`、`pong checkpoint` 或 `pong rollback` CLI。修改持久化语义前，请阅读[开发指南](docs/development/DEVELOPMENT_GUIDE.md)和[下一任务](docs/roadmap/NEXT_TASK.md)。

## 技术原则

- 不可变状态；
- 显式身份与所有权；
- 确定性行为；
- fail-closed validation；
- 崩溃恢复与 durable Operation；
- 显式 Version reference；
- 不报告虚假成功；
- provider-neutral、agent-neutral design。

## 明确边界

Pong 尚未达到生产就绪或企业就绪，也不是完全自治系统或 Git replacement。当前没有受支持的 provider integration、公开 SDK/CLI/MCP endpoint、server mode、remote replication、scheduler、merge/rebase policy、shared writable Workspace policy、Candidate/Approval flow 或 Agent State implementation。凭据和外部副作用始终位于 Core 之外。

## 文档

- [架构](docs/architecture/)
- [架构决策](docs/decisions/)
- [开发与测试门禁](docs/development/)
- [路线图与下一任务](docs/roadmap/)
- [验证证据](artifacts/)

## 下一步

在选择 CLI、HTTP、MCP、SDK 或 provider-specific adapter 之前，先基于已硬化的本地 `AgentControl` 语义边界定义 M4 外部 Agent 协议契约。详见 [NEXT_TASK.md](docs/roadmap/NEXT_TASK.md)。
