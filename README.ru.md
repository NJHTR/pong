# Pong

<p align="center">
  <a href="./README.md">English</a> |
  <a href="./README.zh-CN.md">简体中文</a> |
  <a href="./README.ja.md">日本語</a> |
  <strong>Русский</strong>
</p>

<p align="center">
  <a href="https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml"><img alt="CI" src="https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml/badge.svg?branch=dev"></a>
  <a href="https://github.com/NJHTR/pong/releases/tag/v0.1.0"><img alt="Release v0.1.0" src="https://img.shields.io/badge/release-v0.1.0-2ea44f"></a>
  <a href="https://www.rust-lang.org"><img alt="Rust 1.78+" src="https://img.shields.io/badge/rust-1.78%2B-000000?logo=rust"></a>
  <a href="https://www.apache.org/licenses/LICENSE-2.0"><img alt="License Apache 2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <a href="./docs/roadmap/ROADMAP.md"><img alt="Status Experimental" src="https://img.shields.io/badge/status-experimental-orange"></a>
</p>

> Ориентированная на агентов инфраструктура версионируемых рабочих пространств и выполнений для длительной и многоагентной разработки ПО.

Pong — не оболочка Git для ИИ. Он предоставляет устойчивую модель версий, состояний, задач, рабочих пространств и выполнений агентов. Сейчас проект представляет собой **внутреннее, экспериментальное Rust-ядро с обязательным тестовым контролем**.

## Состояние проекта

| Этап | Состояние |
| --- | --- |
| Текущий выпуск | `v0.1.0` |
| M1 durable primitives | Выпущен |
| M2 state engine | Внутренняя реализация завершена |
| M3 agent execution core | Внутренняя реализация завершена |
| M4 provider-neutral `AgentControl` | Реализован и усилен для надежной публикации; контракт внешнего протокола в работе |

## О проекте

Pong — локальная инфраструктура контроля версий и выполнения для ИИ-агентов. Она дополняет Git и делает рабочие пространства, переходы состояний, контрольные точки, передачу задач и восстановление отслеживаемыми и устойчивыми.

Git управляет историей исходного кода. Pong управляет состоянием и доказательствами, возникающими во время работы агентов. Pong — это библиотека/ядро на Rust, а не среда выполнения модели, планировщик, публичный CLI, сервер, SDK или UI.

## Зачем нужен Pong?

У выполнения агентами есть требования, которые плохо укладываются в ориентированный на человека процесс Git:

- один Agent может запускать несколько SubAgent;
- несколько Execution могут работать параллельно;
- Execution может завершиться ошибкой, аварией, тайм-аутом или исчерпанием квоты;
- Task может передаваться между Codex, Claude, Cursor и другими runtime;
- один Task может создавать множество промежуточных и кандидатных состояний;
- восстановление должно возвращать известное состояние Task без удаления истории.

Pong решает эти задачи с помощью явной идентификации, неизменяемого состояния, durable Operation, детерминированного восстановления и проверяемых границ владения.

## Pong и Git

| Git | Pong |
| --- | --- |
| Commit, branch, merge | Agent Workspace, Snapshot, Version, Operation |
| История совместной работы людей | Устойчивое состояние автоматического выполнения |
| Родословная дерева исходников | Контекст выполнения, ссылки на состояние, данные для восстановления |

Обе системы рассчитаны на совместное использование. Pong не заменяет Git.

## Архитектура

`Workspace.head` — корневой digest Snapshot. Version Head является отдельным логическим выбором и не меняет семантику Snapshot Head.

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

Реализованные уровни:

- **Storage Core:** метаданные SQLite, content-addressed storage, публикация в файловой системе, canonical identity и проверка схемы.
- **State Engine:** жизненный цикл Workspace, Snapshot, restore, diff, reconciliation, Version graph/Head, durable Operation ledger, lease, контроль revision и границы восстановления.
- **Execution Engine:** Agent, Task, Execution, независимые Workspace, Checkpoint, Handoff, Resume, межпространственные materialization/diff/restore/rollback и явный provenance.
- **Control Layer:** локальный Rust-facade `AgentControl` с типизированными запросами и представлениями над устойчивым процессом.

Memory, Skill, Automation, Policy, Plugin, Permission, Candidate и Approval относятся к будущим этапам.

## Выполнение агентами

У каждого Agent есть устойчивая идентичность. Task определяет работу, а Execution представляет одну конкретную попытку одного Agent. Execution явно хранит ссылки на Workspace, base Version, current Version и Operation.

Родительские и дочерние Execution образуют ограниченный по глубине ациклический граф. Это отношение не совпадает с родословной Version или Handoff.

### Многоагентные рабочие пространства

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

Несколько Execution могут начинаться с одной base Version, используя независимые writable Workspace. Родитель Version всегда остается в том же Workspace. Продолжение между Workspace выполняется через явные записи Checkpoint, Handoff, Resume и source Version. Каждая запись защищена lease и revision compare-and-swap.

### Передача и восстановление

Pong сохраняет записи Handoff и Resume, не переписывая идентичность Task и не создавая фиктивную родословную Version. Локальные свидетельства разработки подтверждают передачу на уровне процессов от Codex к Claude Code, но это еще не поддерживаемый provider adapter или удаленный протокол.

Rollback сохраняет историю Version, Operation и Execution. Межпространственный rollback публикует target-local Snapshot, обновляет только head целевого Workspace, сохраняет его Version Head и не меняет исходный Workspace.

## Прогресс

### Завершено

- Выпуск M1 durable primitives: `v0.1.0`.
- M2 state engine и тесты migration, fault и recovery.
- M3 execution records, межпространственные процессы и локальная проверка передачи между реальными агентами.
- Локальный M4 `AgentControl`, включая отклонение stale revision, durable exact retry, восстановление после pre-commit failure и post-commit cold-reopen recovery.

### В работе

- Внешний контракт Agent-протокола M4: transport-neutral request/response, reconnect, observability, authentication ownership и безопасные wire error.

### Запланировано

- внешние provider adapter и transport;
- scheduling и orchestration;
- политика merge, rebase и reconciliation между Execution;
- Candidate и Approval;
- слой Agent State;
- безопасная самомодификация.

## Локальный запуск

Требования:

- Rust `1.78` или более новый совместимый toolchain;
- локальная файловая система, поддерживаемая платформой.

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --all --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
git diff --check
```

Публичных команд `pong init`, `pong checkpoint` и `pong rollback` пока нет. Перед изменением семантики хранения прочитайте [руководство разработчика](docs/development/DEVELOPMENT_GUIDE.md) и [описание следующей задачи](docs/roadmap/NEXT_TASK.md).

## Технические принципы

- неизменяемое состояние;
- явная идентификация и владение;
- детерминированное поведение;
- fail-closed validation;
- восстановление после сбоев и durable Operation;
- явные ссылки Version;
- отсутствие фиктивного успеха;
- provider-neutral и agent-neutral design.

## Явные ограничения

Pong не готов к production или enterprise, не является полностью автономной системой и не заменяет Git. Нет поддерживаемой provider integration, публичного SDK/CLI/MCP endpoint, server mode, remote replication, scheduler, политики merge/rebase, shared writable Workspace policy, потока Candidate/Approval или реализации Agent State. Учетные данные и внешние побочные эффекты остаются за пределами Core.

## Документация

- [Архитектура](docs/architecture/)
- [Архитектурные решения](docs/decisions/)
- [Разработка и тестовые барьеры](docs/development/)
- [Дорожная карта и следующая задача](docs/roadmap/)
- [Данные проверок](artifacts/)

## Следующий шаг

До выбора CLI, HTTP, MCP, SDK или provider-specific adapter необходимо определить внешний контракт Agent-протокола M4 поверх усиленной локальной семантической границы `AgentControl`. Подробнее см. [NEXT_TASK.md](docs/roadmap/NEXT_TASK.md).
