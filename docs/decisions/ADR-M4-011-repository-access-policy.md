# ADR-M4-011: Repository Access Policy

- **Status:** `Accepted policy / Partial platform evidence / Internal M4`
- **Date:** 2026-09-15

## Context

M4-008 showed that independent Windows processes directly opening one
Repository can fail closed with transient codes `2`, `33`, and, in one full
run, protected access code `5`. M4-009 and M4-010 established a single Core
owner and verified multiple real Runtime processes through that authority.

The unresolved issue is architectural: direct Rust storage access and external
Runtime access must not be treated as the same supported mode.

## Decision

Freeze three access modes:

1. Core owned access is the supported external Runtime mode.
2. Direct `Repository` access remains supported for embedded/internal Rust
   integration, tests, and offline maintenance while no Core owns the state.
3. Multiple external Runtime processes directly opening Repository are not
   supported.

The production external Runtime path is protocol -> AgentControl -> Core ->
Repository. Do not remove or make the Repository API Core-only. Do not infer a
multi-process Runtime guarantee from SQLite's connection model.

Keep Mode C tests active as negative safety diagnostics. They must prove
successful durable state or fail-closed behavior and cold-reopen integrity;
they must never accept corruption, partial publication, or silent overwrite.

## Error Decision

Owner-lock contention maps to `CONFLICT`. Windows codes `32` and `33` have
defined sharing/lock meanings. Code `2` is only a context-dependent namespace
race observation. Code `5` is ambiguous because Pong also observes it for real
ACL denial, so this ADR does not globally reclassify it as a direct-access
conflict. Until stronger evidence identifies the failing operation, code `5`
remains `NOT_PROVEN` and visible.

## Release Decision

The supported Core gate is based on Core ownership, Runtime-through-Core,
crash isolation, reconnect, durable recovery, second-Core rejection, and
existing lease/revision/Operation semantics. Direct multi-process external
availability is not a release feature.

Negative safety diagnostics still run in the full suite. A nonzero diagnostic
is not relabeled green; it records a partial policy-frozen development state
until the error is classified or the safety invariant is otherwise proven.

## Consequences

- One Repository has one supported external Runtime state authority.
- Embedded users retain a direct Rust API without a new schema or type layer.
- External Runtime implementations cannot claim support when bypassing Core.
- Existing M4-008 tests are not removed, ignored, or serialized.
- Windows Mode C remains `PARTIAL / IMPLEMENTATION LIMITATION`.
- Linux/macOS parity remains `NOT_PROVEN` without native execution.
- HTTP, MCP, WebSocket, authentication services, cloud sync, replication, and
  consensus remain outside this decision.
