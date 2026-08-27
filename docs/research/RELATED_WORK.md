# Related Work and Evidence

This is a decision-oriented survey. The implementation team should refresh links and versions when a milestone starts; the abstractions below are intentionally described independent of vendor branding.

## Git internals

- **Object model and Merkle-like identity:** blobs, trees, commits, and annotated tags make immutable content and parent relationships cheap to compare. Pong adopts immutable CAS roots and refs, but adds typed manifests and execution references.
- **Index/worktree:** Git separates intended index state from a materialized worktree. Pong adopts the separation between logical snapshot and workspace materialization, while extending the index concept with operation cursors and capture confidence.
- **Hooks:** hooks show that interception at lifecycle boundaries is useful. Pong treats hooks as one adapter path, never as a guarantee that every tool call was observed.
- **Packfiles/LFS:** storage compaction and large-object indirection inform an ObjectStore interface. v0.x uses a simpler filesystem CAS and leaves pack/remote negotiation behind the interface.

## Agent state and trajectory systems

Framework checkpointing typically serializes graph/node state, messages, or task state at framework-defined boundaries. It is valuable for resume and deterministic tests, but usually omits a portable workspace snapshot, process/tool evidence, environment policy, external side effects, and a project-wide agent registry. Pong's checkpoint references native adapter payloads rather than replacing them.

Agent-workspace products that present Git-like snapshots validate the usability of commits, diffs, and rollback for agent work. They do not by themselves settle cross-framework identity, event causality, secret redaction, or irreversible-effect policy. Pong therefore treats that category as UX inspiration and an integration target, not a complete Core specification.

## Execution and storage patterns

- Event sourcing provides an audit-friendly append log; snapshots bound replay cost. Pong uses both with per-stream sequence and idempotent projections.
- Content-addressable/Merkle storage gives integrity and deduplication, but an object hash is not a trust attestation. Provenance and policy metadata remain separate.
- Overlay/COW filesystems provide efficient isolation but differ across hosts. Pong defines a driver capability contract and a portable file-operation fallback.
- Local SQLite-like transactional metadata plus a filesystem CAS balances offline operation, inspectability, and migration in v0.x. PostgreSQL/RocksDB/object storage remain later implementations of the same store ports.

## Evidence sources

These primary or maintained references anchor the conclusions above. They are research inputs, not dependencies of Pong's runtime:

- Git object model, refs, index, packfiles, hooks, and worktrees: [Git Internals](https://git-scm.com/book/en/v2/Git-Internals-Plumbing-and-Porcelain) and [gitglossary](https://git-scm.com/docs/gitglossary).
- Git Large File Storage: [Git LFS](https://git-lfs.com/).
- Event sourcing: [Martin Fowler, Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html).
- LangGraph persistence and checkpoints: [LangGraph persistence](https://docs.langchain.com/oss/python/langgraph/persistence).
- AutoGen agent state and messaging: [AutoGen AgentChat](https://microsoft.github.io/autogen/stable/user-guide/agentchat-user-guide/index.html).
- CrewAI task, agent, and memory concepts: [CrewAI documentation](https://docs.crewai.com/).
- OpenHands runtime and workspace model: [OpenHands documentation](https://docs.all-hands.dev/).
- Container isolation and checkpointing constraints: [Docker checkpoint and restore](https://docs.docker.com/reference/cli/docker/checkpoint/).
- Content-addressed storage and Merkle data structures: [IPFS Concepts](https://docs.ipfs.tech/concepts/).

The sources establish patterns and constraints, not proof that Pong can provide equivalent guarantees. Adapter and storage PoCs remain required.

## Research gaps to validate with PoCs

1. Crash ordering when a tool succeeds between pre-call and post-call capture.
2. Completeness and cost of filesystem/process/browser interception across supported runtimes.
3. Portable environment fingerprinting without leaking secrets or host identity.
4. Size and latency of operation/event payloads for realistic agent sessions.
5. Whether a useful three-way merge can combine independent operation streams beyond file content.
