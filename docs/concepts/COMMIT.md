# Commit

**Status: Normative.** A Commit is an immutable, content-addressed semantic version node in the version DAG. It points to a root snapshot, zero or more parents, operation range, authoring agent/session, message, task context, and safe metadata.

Commits are created explicitly or by a declared checkpoint-to-commit policy; an operation never silently becomes a commit. The commit hash covers canonical header, parent IDs, snapshot root, and referenced manifests. It does not contain secrets or mutable environment values.

A commit describes a coherent project state for comparison and collaboration. It is not a guarantee that every external side effect can be undone, nor does it imply that the producing agent can resume execution. Use a Checkpoint for that recovery contract.

