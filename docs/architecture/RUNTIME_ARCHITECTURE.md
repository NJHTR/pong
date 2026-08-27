# Runtime Architecture

## Capture pipeline

```text
tool request
  -> context validation
  -> policy check and secret redaction plan
  -> before evidence (when possible)
  -> tool execution
  -> after evidence and result classification
  -> durable operation record
  -> event append and projections
  -> caller response
```

The runtime assigns an operation id before execution and a capture sequence on append. If the process crashes, recovery reconciles in-flight records using the operation journal and provider evidence.

## Generic interception

v0.x can reliably wrap Pong SDK calls, filesystem APIs in a managed workspace, process launchers, and explicit artifact writes. Shell commands are captured at the wrapper boundary; child processes and privileged escapes may be only partially observed. Filesystem watchers are reconciliation aids, not the sole audit source.

## Framework-specific interception

Adapters receive agent lifecycle, task, checkpoint, message, and model-state hooks. They can attach framework run ids and serialized state references, but must not write arbitrary framework state into the core schema. Missing hooks lower capture confidence rather than inventing events.

## Operation classification

Each operation declares:

- `reversible`: a compensating action or snapshot restore exists.
- `replayable`: inputs and required context are sufficient to attempt it again.
- `side_effect`: local, controlled external, or irreversible external.
- `capture_confidence`: complete, partial, inferred, or missing.

Examples: managed file write is reversible and replayable; web search is replayable with recorded request metadata but results may differ; payment is irreversible and never auto-replayed.

## Rollback and replay policy

Rollback changes selected recoverable state, normally workspace filesystem and Pong refs. It does not undo external effects or erase events. Replay creates a new operation lineage and requires policy approval for anything beyond deterministic local actions. Tool output is treated as evidence, not as a promise of identical future output.

## Runtime modes

1. **Embedded**: library hooks execute in the agent process.
2. **Sidecar**: a local service receives signed operation envelopes.
3. **Provider**: a workspace provider supplies filesystem/process isolation and reconciliation.

All modes emit the same domain events and preserve operation ids across retries.
