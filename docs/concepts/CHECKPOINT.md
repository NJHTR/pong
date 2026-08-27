# Checkpoint

**Status: Normative.** A Checkpoint is an immutable recovery contract for resuming an agent session. It references a snapshot, operation cursor, agent state adapter payload, task context, environment fingerprint, and replay/approval boundary.

Checkpoint creation is explicit or adapter-triggered at a declared safe point. It is valid only when all required referenced objects are durable and its policy says which external effects are excluded. Restoring a checkpoint can recreate a workspace and agent context, but cannot roll back an already-observed irreversible side effect. Restore therefore produces a new session and an audit event.

Checkpoint and commit may share the same snapshot root, but neither implies the other. A commit optimizes for semantic collaboration; a checkpoint optimizes for safe continuation.

