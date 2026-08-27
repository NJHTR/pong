# Competitive and Design Analysis

**Status: Normative research input.** This document records design conclusions rather than a bibliography. Product and API decisions must cite the relevant row or ADR.

## Comparison

| System/pattern | What it solves well | What Pong can adopt | What Pong must add or reject |
| --- | --- | --- | --- |
| Git object model, refs, index, worktree | Immutable content graph, cheap branching, explicit semantic commits | Canonical object IDs, parent DAG, refs, index-like materialization manifest, hooks as integration points | Git assumes files and human-authored commands; it does not model tool intent, agent identity, environment, side effects, or recovery state |
| Git packfiles and LFS | Deduplication and large-object transfer | Separate immutable CAS from metadata, chunk/pack later, artifact manifests | Do not make Git's wire/storage layout a Core contract; execution payloads have sensitivity and retention policies |
| AgentGit-style repositories | Git-like checkpoints for agent workspaces/trajectories | Familiar commit/checkpoint UX and workspace snapshots | A file tree alone misses process state, tool calls, external effects, policy, and multi-agent causality; validate exact project/version compatibility before claiming feature parity |
| Event sourcing + snapshots | Rebuildable history and bounded recovery cost | Append-only event log, periodic materialized snapshots, idempotent consumers | Events are not automatically reversible; compaction must preserve audit/provenance and causal links |
| Merkle DAG/CAS | Integrity, deduplication, deterministic references | Content-addressed snapshots, manifests, commit roots | Hashing cannot prove an external effect happened or make an unsafe payload safe |
| OverlayFS/COW/container snapshots | Fast isolated filesystem branches | Driver interface, copy-on-write where available, content diff fallback | Kernel/container behavior varies; a snapshot is not a portable environment or database transaction |
| LangGraph checkpoint/persistence | Graph execution state at resumable points | Adapter contract for state serialization, run/thread IDs, checkpoint lineage | Framework graph semantics stay in adapter; Pong must record filesystem, environment, operations, and cross-framework identity |
| AutoGen/CrewAI state | Agent/task messages and framework lifecycle | Lifecycle adapter hooks and correlation IDs | Do not make framework message schemas a Core protocol; messages are context, not a queue |
| OpenHands/Claude Code/Codex-style workspaces | Practical tool execution, sandboxes, human-in-loop interaction | Generic tool interception and driver capability model | Tool surfaces differ and may bypass wrappers; capture confidence and provide reconciliation instead of pretending universal visibility |
| Browser/session persistence | Cookies, pages, and navigation context | Redacted browser adapter artifacts and session fingerprints | Credentials and remote state stay outside ordinary commits; replay requires policy and may be non-deterministic |
| SQLite/PostgreSQL/RocksDB/object storage | Different durability, query, scale, and binary-object tradeoffs | SQLite local metadata/event WAL plus filesystem CAS for v0.x; ports behind interfaces | Do not couple Core to one database; distributed coordination and object lifecycle need later stores |

## Findings

1. **Git is a substrate, not the product.** Pong should borrow the immutable DAG, refs, object identity, and explicit commit semantics while treating execution as a second graph.
2. **A single trajectory format is insufficient.** Framework checkpoint systems persist internal state but do not provide a cross-framework workspace and side-effect ledger. Pong adapters must normalize identity and causality, not erase native state.
3. **Capture is an evidence problem.** Filesystem watchers, wrappers, and middleware have blind spots. Every record therefore carries capture method and confidence (`enforced`, `wrapped`, `observed`, `declared`, `unknown`); unknown is not silently promoted to observed.
4. **Local-first is the right v0.x constraint.** A durable local WAL and CAS can be tested deterministically and works offline. Remote sync is a transport/replication concern, not a prerequisite for the object model.
5. **Reversibility cannot be inferred from version history.** Compensation, replay, approval, and secret policy are explicit operation metadata and policy decisions.

## Differentiation hypothesis

Pong's durable wedge is the normalized, causally linked record joining **agent identity -> workspace/environment -> operation/tool evidence -> version DAG -> recovery/checkpoint policy**, while remaining framework-agnostic. That hypothesis must be tested with cross-framework and crash-recovery PoCs before implementation scope expands.

The comparison is grounded in the primary references listed in [`RELATED_WORK.md`](RELATED_WORK.md); product names are treated as moving integration targets, not fixed Core dependencies.
