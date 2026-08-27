# Execution Model

## Two graphs

Pong maintains:

1. A **version DAG** of immutable commits and parent relationships.
2. An **execution DAG** of tasks, operations, events, checkpoints, agent handoffs, and artifact derivations.

The graphs are linked by provenance ranges and snapshot references, but neither is a substitute for the other.

## Operation lifecycle

`planned (optional) -> started -> completed | failed | cancelled | unknown`.

An operation id is allocated before the tool call. Start and terminal events are append-only. `unknown` means the tool may have executed but the recorder lacks a definitive result; it blocks automatic replay unless policy resolves the uncertainty.

## Ordering and causality

Each operation has a parent operation or causal predecessor when known, a monotonic local sequence, and a wall-clock timestamp. Timestamps are informational; sequence and explicit parent links define ordering. Cross-agent order is represented by handoff or message events and is not inferred from clocks.

## Reversible, replayable, irreversible

- **Reversible**: Pong can restore controlled state or apply a verified compensating action.
- **Replayable**: Pong can attempt the operation again from recorded inputs and context; output may differ.
- **Irreversible**: external effect cannot be safely undone by Pong.

Categories are independent. An operation may be replayable but not reversible, or reversible but not replayable. Policies combine category, capability, target, and approval.

## Commit and checkpoint semantics

A commit captures a coherent workspace tree and semantic intent. A checkpoint captures enough workspace and execution state to resume a task, including a cursor and replay exclusions. A checkpoint can reference an uncommitted snapshot and does not move a branch ref.

## Failure and recovery

On crash, recovery scans started operations, verifies provider evidence, and emits reconciliation events. It never fabricates success. A workspace rollback restores selected local state to a snapshot or commit; it does not erase events, rewind external systems, or reset another agent's process memory.

## Merge

Merging combines version DAG states with explicit conflict records. Execution histories remain separate and are linked to the merge operation. Semantic conflicts may require the agent framework or human to resolve.
