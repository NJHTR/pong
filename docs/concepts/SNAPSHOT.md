# Snapshot

**Status: Normative.** A Snapshot is an immutable factual capture of a selected state at a point in time. Its canonical manifest names tree/resource roots, environment revision, operation cursor, capture confidence, and redaction profile.

The current internal M2 implementation supplies a filesystem-only tree
manifest: bytewise-sorted portable paths, directory entries, and CAS digests for
regular-file blobs. The manifest is bound to workspace/project identity and the
active redaction profile. This is a bounded implementation subset, not yet the
full environment/resource/operation snapshot contract described below.

Snapshots may be transient, uncommitted, or referenced by a checkpoint. They are content-addressed and can be compared or materialized, but they do not carry semantic intent, move a branch, or promise agent resumability. A Snapshot that contains unknown or unreconciled resources says so in its manifest; it is not silently treated as complete.

Materialization currently writes only into a new destination through a temporary
sibling directory and atomic rename. Existing destinations conflict; failed
materialization is cleaned up without deleting user data. Crash reconciliation,
incremental snapshots, and non-filesystem resources are not implemented.
