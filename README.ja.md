# Pong

<p align="center">
  <a href="./README.md">English</a> |
  <a href="./README.zh-CN.md">简体中文</a> |
  <strong>日本語</strong> |
  <a href="./README.ru.md">Русский</a>
</p>

<p align="center">
  <a href="https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml"><img alt="CI" src="https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml/badge.svg?branch=dev"></a>
  <a href="https://github.com/NJHTR/pong/releases/tag/v0.1.0"><img alt="Release v0.1.0" src="https://img.shields.io/badge/release-v0.1.0-2ea44f"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.78+" src="https://img.shields.io/badge/rust-1.78%2B-000000?logo=rust"></a>
  <a href="https://www.apache.org/licenses/LICENSE-2.0"><img alt="License Apache 2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <a href="./docs/roadmap/ROADMAP.md"><img alt="Status Experimental" src="https://img.shields.io/badge/status-experimental-orange"></a>
</p>

> 長時間実行およびマルチエージェント開発のための、エージェント中心のバージョン管理されたワークスペース／実行基盤。

Pong は AI に Git をかぶせるラッパーではありません。エージェントのバージョン、状態、タスク、ワークスペース、実行を永続的に管理するモデルを提供します。現在は**内部向け・実験的・テストゲート付き**の Rust Core です。

## プロジェクト状況

| マイルストーン | 状況 |
| --- | --- |
| 現在のリリース | `v0.1.0` |
| M1 durable primitives | リリース済み |
| M2 state engine | 内部実装完了 |
| M3 agent execution core | 内部実装完了 |
| M4 provider-neutral `AgentControl` | 実装および publication hardening 完了、外部プロトコル契約を策定中 |

## 概要

Pong はローカルファーストな AI エージェント向けバージョン管理／実行基盤です。Git と共存しながら、エージェントのワークスペース、状態遷移、Checkpoint、Handoff、復旧を追跡可能かつ永続的にします。

Git はソースコードの履歴を管理し、Pong はエージェント実行中に生まれる状態と証拠を管理します。Pong は Rust の library/core であり、モデルランタイム、スケジューラー、公開 CLI、サーバー、SDK、UI ではありません。

## なぜ Pong が必要か

エージェント実行には、人間中心の Git ワークフローだけでは表現しにくい要件があります。

- 1 つの Agent が複数の SubAgent を起動できる。
- 複数の Execution が並行して作業できる。
- Execution は失敗、クラッシュ、タイムアウト、クォータ枯渇を起こし得る。
- Task は Codex、Claude、Cursor、その他の runtime 間で引き継がれる。
- 1 つの Task が多数の中間状態や候補状態を生成する。
- 履歴を削除せず、既知の Task 状態へ復旧できなければならない。

Pong は明示的な ID、不変状態、durable Operation、決定的な復旧、監査可能な所有権境界でこれらに対応します。

## Pong と Git

| Git | Pong |
| --- | --- |
| Commit、branch、merge | Agent Workspace、Snapshot、Version、Operation |
| 人間の協働履歴 | 自動実行の永続状態 |
| ソースツリーの系譜 | 実行コンテキスト、状態参照、復旧証拠 |

両者は併用する設計です。Pong は Git の代替ではありません。

## アーキテクチャ

`Workspace.head` は Snapshot の root digest です。Version Head は独立した論理選択であり、Snapshot Head の意味を変更しません。

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

実装済みのレイヤー：

- **Storage Core:** SQLite metadata、content-addressed storage、ファイルシステムへの publication、canonical identity、schema validation。
- **State Engine:** Workspace lifecycle、Snapshot、restore、diff、reconciliation、Version graph/Head、durable Operation ledger、lease、revision guard、復旧境界。
- **Execution Engine:** Agent、Task、Execution、独立 Workspace、Checkpoint、Handoff、Resume、Workspace 間の materialization/diff/restore/rollback、明示的 provenance。
- **Control Layer:** durable workflow を typed request/view で構成する、ローカル Rust `AgentControl` facade。

Memory、Skill、Automation、Policy、Plugin、Permission、Candidate、Approval は後続マイルストーンの対象です。

## Agent Execution

各 Agent は永続 ID を持ちます。Task は作業を識別し、Execution は 1 つの Agent による具体的な 1 回の試行を表します。Execution は Workspace、base Version、current Version、Operation の参照を明示的に保持します。

親子 Execution は深さが制限された非循環の実行グラフを構成します。この関係は Version ancestry および Handoff とは別です。

### マルチエージェント Workspace

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

複数の Execution は同じ base Version から開始しつつ、独立した writable Workspace を利用できます。Version parent は常に同一 Workspace 内に留まります。Workspace をまたぐ継続は、明示的な Checkpoint、Handoff、Resume、source Version record で行います。すべての書き込みは lease と revision compare-and-swap で保護されます。

### 引き継ぎと復旧

Pong は Task identity を書き換えたり Version ancestry を偽造したりせず、Handoff と Resume record を永続化します。ローカル開発証拠では Codex から Claude Code へのプロセス単位の引き継ぎを検証済みですが、これはまだサポート対象の provider adapter やリモートプロトコルではありません。

Rollback は Version、Operation、Execution の履歴を保持します。Workspace 間 rollback は target-local Snapshot を発行し、対象 Workspace の head のみを更新し、Version Head を維持し、source Workspace を変更しません。

## 進捗

### 完了

- M1 durable primitives release: `v0.1.0`。
- M2 state engine と migration、fault、recovery のテスト。
- M3 execution records、Workspace 間 workflow、ローカル実 Agent handoff 検証。
- M4 ローカル `AgentControl`。stale revision の拒否、durable exact retry、pre-commit failure recovery、post-commit cold-reopen recovery を含みます。

### 進行中

- M4 外部 Agent プロトコル契約：transport-neutral request/response、reconnect、observability、authentication ownership、安全な wire error。

### 計画中

- 外部 provider adapter と transport。
- scheduling と orchestration。
- Execution 間の merge、rebase、reconciliation policy。
- Candidate と Approval。
- Agent State layer。
- 安全な自己進化。

## ローカル実行

要件：

- Rust `1.78` または互換性のある新しい toolchain。
- ホストプラットフォームでサポートされるローカルファイルシステム。

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --all --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
git diff --check
```

現在、公開された `pong init`、`pong checkpoint`、`pong rollback` CLI はありません。永続化セマンティクスを変更する前に、[開発ガイド](docs/development/DEVELOPMENT_GUIDE.md)と[次のタスク](docs/roadmap/NEXT_TASK.md)を確認してください。

## 技術原則

- 不変状態。
- 明示的な ID と所有権。
- 決定的な動作。
- fail-closed validation。
- クラッシュ復旧と durable Operation。
- 明示的な Version reference。
- phantom success を許さない。
- provider-neutral、agent-neutral design。

## 明確な制限

Pong は production-ready、enterprise-ready、完全自律システム、Git replacement ではありません。サポート対象の provider integration、公開 SDK/CLI/MCP endpoint、server mode、remote replication、scheduler、merge/rebase policy、shared writable Workspace policy、Candidate/Approval flow、Agent State implementation はまだありません。credential と外部副作用は Core の外部に置かれます。

## ドキュメント

- [アーキテクチャ](docs/architecture/)
- [Architecture Decision](docs/decisions/)
- [開発とテストゲート](docs/development/)
- [ロードマップと次のタスク](docs/roadmap/)
- [検証証拠](artifacts/)

## 次のステップ

CLI、HTTP、MCP、SDK、provider-specific adapter を選択する前に、hardening 済みのローカル `AgentControl` セマンティック境界上で M4 外部 Agent プロトコル契約を定義します。詳細は [NEXT_TASK.md](docs/roadmap/NEXT_TASK.md) を参照してください。
