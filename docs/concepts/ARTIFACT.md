# Artifact

**Status: Normative.** An Artifact is a typed, digest-addressed output or input that is too large, binary, external, or semantically distinct to embed in an event or commit header. Examples include images, logs, model traces, patches, reports, and tool payloads.

An artifact manifest records digest, media type, size, producer operation, project/workspace scope, retention class, provenance, and sensitivity label. A tracked artifact enters a commit through its manifest; an ephemeral artifact may remain event-referenced without becoming part of the version DAG. Content is immutable; logical names are mutable metadata.

