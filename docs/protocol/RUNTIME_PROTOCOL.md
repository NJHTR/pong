# Runtime Protocol

## Purpose

Runtime Protocol defines how an execution environment reports tool calls and lifecycle state to Pong. It supports implicit interception while preserving an explicit adapter API for frameworks that can provide richer context.

## Interception tiers

1. **Native wrappers**: filesystem, process, HTTP, browser, database, and artifact SDK wrappers provide structured before/after data.
2. **Sandbox observers**: OS audit hooks, filesystem journals, and process monitors provide best-effort discovery when wrappers are bypassed.
3. **Framework adapters**: LangGraph, AutoGen, CrewAI, OpenHands, and custom runtimes attach task, node, and message semantics.
4. **Reconciliation**: a post-run scan compares workspace manifests and reports unobserved changes.

No tier claims visibility it cannot prove. Each operation carries `capture_mode` (`native`, `adapter`, `observer`, `reconciled`, `declared`) and confidence.

## Runtime handshake

`runtime.hello` advertises runtime ID, host/container identity, supported hooks, workspace mount, clock info, and protocol version. `runtime.attach` binds the stream to an agent/workspace. Pong returns a lease epoch, redaction policy, sampling limits, and backpressure behavior.

## Operation lifecycle

```text
operation.started -> zero or more operation.progress ->
operation.finished | operation.failed | operation.cancelled | operation.unknown
```

The start record includes tool/action, normalized input digest, resource claims, parent operation, and side-effect classification. Finish includes output digest, result, state deltas, and exit metadata. Raw input/output is stored only when policy permits; otherwise a digest and summary are retained.

## File and process semantics

File operations record normalized path, precondition hash, resulting hash, byte range (when applicable), and whether content was redacted. Shell/process operations record argv or a redacted command digest, working directory, environment profile ID, exit code, signals, and inherited operation context. Secrets and full environment snapshots are excluded by default.

## Network, browser, database, and generation

HTTP/browser events record destination origin, method/action class, request/response digests, status, and approval decision; bodies require explicit policy. Database events record logical operation, target classification, transaction ID, and row-count/hash summaries, never unrestricted credentials. Image or artifact generation records provider/model metadata, prompt digest, seed when available, output artifact IDs, and replayability; provider calls are replayable only if the provider supports deterministic or cached replay.

## Backpressure and loss

The runtime uses a bounded local journal as the source of truth. When the consumer is unavailable, events queue locally up to a configured limit. On overflow, low-value progress events may be sampled, but lifecycle and mutation records are never silently dropped; the runtime marks the stream degraded and emits a loss marker. A dropped event cannot be reconstructed as if observed.

## Shutdown and crash handling

`runtime.flush` requests durable upload of the journal. Graceful shutdown emits `runtime.stopped`; crash recovery scans unfinished operations and marks them `unknown` until reconciled. Runtime restart uses the same stream ID with a new epoch and resumes from the last acknowledged sequence.

## Adapter contract

Framework adapters map framework run/node/message IDs to Pong task and operation IDs, provide state checkpoints, and declare capabilities. They must be optional, versioned independently, and unable to mutate core history outside the standard command path.

