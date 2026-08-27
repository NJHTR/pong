# Replay

**Status: Normative.** Replay is a controlled re-execution or reconstruction of an operation range from a checkpoint/snapshot boundary. It is not a promise of identical external results.

Pong first classifies each operation, validates permissions and side-effect policy, and builds a replay plan. Deterministic operations may be reconstructed from captured inputs and object versions. Replayable operations are re-invoked in an isolated workspace with approval and provenance. Irreversible operations are never silently re-issued; they require an explicit idempotency key and human or policy approval, or are represented as skipped attestations.

Replay creates a new session and operation lineage. It never rewrites the original DAG or event log. Divergent outputs are recorded as new artifacts and operations, with a comparison report against the source run.

