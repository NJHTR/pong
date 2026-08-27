# Development Guide

## Principles

Documentation and ADRs precede implementation. Core remains framework-agnostic, local-first, observable, and explicit about irreversible effects. Changes are small, reviewable, and tied to an invariant or user workflow.

## Repository shape

Keep protocol, security, reliability, compatibility, and development docs under `docs/`. The authorized internal implementation uses the Rust Core crate and a thin `rusqlite` metadata adapter selected by ADR-0013. M1 remains release-gated; bounded M2 workspace/snapshot development follows `M2_WORKSPACE_SNAPSHOT_GATE.md`. Formal implementation remains separated into core domain, storage, runtime adapters, SDK, and CLI. Experimental validation belongs in `poc/` and must be labeled non-production.

## Change workflow

Read `docs/roadmap/NEXT_TASK.md`, inspect relevant architecture and protocol docs, write or update an ADR for consequential decisions, design tests, implement the smallest coherent slice, run verification, and update changelog/roadmap. Never silently change a public field, event, or permission.

## Data and API conventions

Use opaque typed IDs, RFC 3339 timestamps, canonical JSON for hashes, explicit schema versions, cursor pagination, structured error codes, and request IDs. Paths are relative to a workspace root. Secrets are redacted before logs, fixtures, snapshots, or examples.

## Observability

Every operation has start/terminal evidence or an explicit unknown state. Metrics cover journal lag, dropped/sampled events, reconciliation count, conflicts, recovery time, and authorization denials. Logs include correlation IDs but not raw secrets.

## Local-first constraints

The local journal and object store are authoritative in v0.x. Remote synchronization is an adapter and must tolerate offline operation, duplicate delivery, and eventual consistency. Do not add a network dependency to core workflows without an ADR.

## Rust workflow

Use the toolchain version recorded by the implementation CI/release manifest. Run `cargo fmt --check`, `cargo check`, and `cargo test` before claiming a primitive is usable. A failed dependency fetch, platform-specific filesystem test, or M1/M2 gate row is a blocked result, not a reason to weaken the contract. Python and Node clients must use the versioned protocol and must not open `.pong/metadata.sqlite`.

## Review checklist

Reviewers verify invariants, failure paths, permission checks, redaction, idempotency, migration behavior, concurrency assumptions, and documentation updates. Prefer deterministic fixtures and property-based tests for hashes, ordering, and merge behavior.
