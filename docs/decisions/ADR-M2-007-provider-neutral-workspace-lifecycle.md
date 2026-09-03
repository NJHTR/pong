# ADR-M2-007: Provider-Neutral Workspace Lifecycle Contract

**Status:** Proposed / internal / test-gated
**Date:** 2026-09-02
**Scope:** M2-SLICE-007 only

## Context

Pong is Git-like infrastructure for AI Agent execution state. M2 owns the
local workspace and snapshot boundary, while future providers may own other
workspace materializations. The core must expose stable logical lifecycle
facts without importing provider paths, handles, or provider-specific policy.

## Problem

The existing local workspace implementation has durable metadata, leases,
snapshots, restore, status, and diff, but no provider-neutral lifecycle
vocabulary or capability boundary. Adding provider behavior directly to the
core would make filesystem details part of the contract and would risk
changing M1 event, projection, generation, or sequence invariants.

## Goals

- Define an internal, provider-neutral lifecycle transition contract.
- Reuse the existing durable workspace state vocabulary and metadata rows.
- Make capability support explicit for snapshot, restore, diff, and status.
- Keep provider side effects and core durability failure semantics distinct.
- Prove the boundary with deterministic, test-gated integration coverage.

## Non-goals

This slice does not add a public SDK or CLI, provider registry, plugin loading,
remote/cloud/Git provider, version/branch/merge semantics, physical deletion,
background reconciliation, or a new SQLite schema/state enum. It does not
accept or modify ADR-0015 or ADR-0016.

## Core responsibilities

The core owns logical identity, the existing durable workspace row, lease and
revision guards, lifecycle transition validation, capability reporting,
operation/event ordering, generation identity, redaction, and recovery
classification. The provider-neutral context contains only these logical
facts. The core must never infer a provider side effect from a returned
status string.

## Provider responsibilities

A provider owns materialization, provider handles, path/URI interpretation,
filesystem or remote cleanup, and provider-specific metadata. The current
`local` driver remains the only production adapter. The contract tests use an
explicit `TEST_DOUBLE`; no real non-local provider is claimed by this ADR.

## Workspace identity

`WorkspaceIdentity` is the tuple `(workspace_id, project_id, provider)`. The
workspace ID is logical and project-scoped. A locator/path is not identity and
is intentionally absent from `WorkspaceProviderContext`. Provider-specific
metadata must not cross this boundary.

## Lifecycle state

The durable state set is exactly the existing seven metadata values:

`created`, `preparing`, `ready`, `active`, `paused`, `reconciling`, and
`archived`.

No `open`, `busy`, `failed`, or `closed` state is introduced. `Open` is a
handle-acquisition/read-like action and does not change durable status.

## Lifecycle transitions

`transition_workspace_lifecycle` is a pure validator. It permits idempotent
retries where the existing state already expresses the requested action. In
particular, `Open` preserves every non-archived state, `Close` logically
archives a non-archived workspace, `BeginCapture`/`CompleteCapture` move
`created -> preparing -> ready`, `Activate`/`Pause`/`Resume` cover execution,
and `BeginRecovery`/`CompleteRecovery` cover `reconciling -> ready`.
Archived workspaces accept only idempotent `Close`; reconciling workspaces
accept only recovery actions. Physical directory cleanup is never implied by
`Close`.

## Capability model

`WorkspaceCapabilities` is a four-boolean model for exactly `snapshot`,
`restore`, `diff`, and `status`. `WorkspaceCapabilities::local()` enables all
four. An unsupported request fails with the stable `UNSUPPORTED` domain error;
capability negotiation does not mutate metadata.

## Operation semantics

Lifecycle actions are logical intent. When an action has a durable operation,
it uses the existing operation ledger and event envelope, including project,
workspace, generation, causation/correlation, redaction, and ordered
sequence fields. Providers cannot allocate project sequences or append core
events directly. The provider boundary cannot mutate `project_sequence`,
generation, projection state, or event order.

## Error boundary

The core exposes `PongError` domain classes. Invalid transitions are
`CONFLICT` (or `RECOVERY_REQUIRED` while reconciling); unsupported capabilities
are `UNSUPPORTED`. Provider errors remain provider results and must be
translated before persistence; provider-specific messages and metadata are not
trusted core identity.

## Durability boundary

The core persists intent and authoritative state before acknowledging durable
success. A provider action succeeding while core persistence fails is not a
successful lifecycle result and must be retried/reconciled. Conversely, core
persistence succeeding while a provider action fails must be recorded as a
failure or recoverable uncertainty, never as an unqualified success. This ADR
does not add a distributed transaction protocol.

## Idempotency

Retries use the existing operation/request identity and deterministic input
envelope. Repeating the same action returns the same logical result or the
same classified terminal state. A changed envelope under an existing request
identity remains an idempotency-key reuse error.

## Recovery

An interrupted action is reopened through the existing repository recovery and
operation ledger. `reconciling` is a fail-closed state until an explicit
recovery action completes. Provider verification may be required, but the
core does not guess whether an external side effect occurred.

## Compatibility with v0.1.0

M1 `v0.1.0` remains immutable. Legacy repositories do not require provider
metadata and remain readable through the existing compatibility boundary.
The lifecycle model is additive and internal; it does not change the legacy
SQLite schema or old-reader claim.

## Testing

`tests/workspace_lifecycle_contract.rs` covers L1-L15: lifecycle transitions,
idempotency, provider success/failure, persistence failure classification,
interrupted reopen, unsupported capabilities, read-only status/diff, legacy
readability, provider-context isolation, deterministic operation retry, and
event/generation immutability. Provider behavior in these tests is a
synthetic `TEST_DOUBLE`, not non-local qualification evidence.

## Future extension

Future provider adapters may implement a separately accepted contract for
materialization and reconciliation. Such work requires a new ADR and must
preserve this core boundary. Public Rust, Python, Node.js, CLI, remote
replication, and version/branch/merge contracts remain later roadmap items.
