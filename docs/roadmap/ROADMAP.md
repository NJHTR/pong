# Pong Roadmap

**Status: Normative planning baseline.** The roadmap follows architectural dependencies, not the order of the initial idea list. Each milestone exits through tests, documentation, and an ADR when a contract changes.

## Phase 0: Research and architecture (complete)

- Establish scope, boundaries, terminology, object relationships, and risks.
- Decide local-first storage, logical workspaces, operation envelope, event model, security boundary, and two-graph execution model.
- Deliver the Phase 0 documents, ADR set, review, constitution, and PoC gates.

## Phase 1: Durable primitives and object identity (release gate open)

- Implement canonical IDs, schema versions, content-addressed objects, manifests, metadata transactions, and append-only event/WAL plumbing.
- Prove atomic publication across metadata and objects, crash recovery, redaction, and idempotent projections.
- No user-facing CLI promise until these invariants are tested.
- Current status: the Rust crate has Windows and Linux Rust 1.78 executable
  evidence for the primitive, recovery, migration, generated-property, host
  ACL/ENOSPC, and measurement harnesses described in `M1_EVIDENCE.md`. The
  M1 release gate is still open because the separately released old-binary
  matrix, accepted platform/capacity budgets, complete host fault matrix, and
  accepted supported-filesystem matrix remain unresolved. The historical
  Windows selector/journal classification regression is fixed for the
  exercised paths, but broader directory-sync coverage still needs acceptance.
  The release audit is the active task; no later phase may be promoted by
  documentation alone.

## Phase 2: Snapshot and local workspace driver (frozen until M1 passes)

- Define portable tree snapshots and materialization manifests.
- Implement the local filesystem driver, workspace lifecycle, leases, attach/migrate records, and safe environment fingerprint.
- Validate path traversal, permissions, partial writes, and reconciliation after crashes.
- Current status: logical workspace/environment metadata, epoch leases, a local filesystem driver, canonical file/directory tree snapshots in CAS, and new-directory materialization exist behind the internal Rust library. Lifecycle breadth, reconciliation, schema-evolution evidence, fault matrices, large-tree budgets, and a public provider contract remain incomplete. The slice is frozen until M1 passes and does not waive the M1 release gate.

## Phase 3: Operations and execution history (frozen until M1 passes)

- Implement the versioned operation envelope, capture confidence, parent/correlation links, artifact references, and task/session registry.
- Add generic filesystem/process wrappers and explicit recording API; publish the supported capture matrix.
- Keep browser/HTTP/DB and framework adapters behind experimental interfaces until PoCs pass.
- Current status: the first internal ledger slice exists below Runtime, SDK,
  CLI, server, and framework adapters. Further work is frozen while M1 and M2
  release gates remain open.

## Phase 4: Versioning and collaboration core

- Implement branches, compare-and-swap refs, semantic commits, diff/log/inspect projections, and conflict records.
- Add checkpoint creation/restore metadata and agent registry queries.
- Test independent workspaces, merge safety, stale heads, and event ordering.

## Phase 5: Policy-gated recovery and replay

- Implement rollback plans for controlled workspace state, checkpoint restore, replay planning, approval, idempotency keys, and irreversible-effect skips.
- Never claim rollback of external state without a resource adapter contract and evidence.
- Run destructive-effect simulation and crash/reconciliation tests before enabling defaults.

## Phase 6: Stable Agent API, protocol, and CLI

- Freeze v0.1 API/protocol schemas through compatibility fixtures.
- Add Python SDK, local protocol transport, and the designed CLI commands with agent/human output modes.
- Provide migration/version negotiation and audit-friendly diagnostics.

## Phase 7: Framework integrations

- Implement adapters one at a time for LangGraph, AutoGen, CrewAI, OpenHands, and a generic runtime.
- Preserve native state payloads, map run/session IDs, and publish adapter-specific capture gaps.
- Each adapter must pass cross-framework fixture and security tests.

## Phase 8: Additional drivers and observability

- Add container/sandbox/remote workspace drivers, browser/HTTP/DB capture where enforcement is available, and pack/large-artifact storage.
- Add activity views and operational metrics without changing Core semantics.

## Phase 9: Replication and optional service mode

- Design and test object/event replication, conflict detection, retention, encryption, and multi-host agent registry.
- Add a service/UI only after local semantics and security claims remain identical.

## Release gates

Every phase requires: updated normative docs, tests written before implementation for new invariants, migration notes, threat-model review, and an explicit list of unsupported capture paths. "Works on one framework" is not a Core release criterion.
