# ADR-0012: Policy-Gated Replay with New Lineage

- Status: Accepted
- Date: 2026-08-19

## Context

Replay can restore useful work, but external calls, generated outputs, time, and credentials may not be deterministic or reversible.

## Problem

How should `replay` behave without duplicating side effects or rewriting history?

## Options

1. Re-run every recorded tool call automatically.
2. Never replay; only restore files.
3. Classify operations, isolate replay, require approval/idempotency for side effects, and create new lineage.

## Decision

Choose option 3. Reversible operations may be reconstructed; replayable ones are re-invoked with provenance; irreversible ones are skipped or explicitly approved with an idempotency key. Original events remain immutable.

## Consequences

Replay is safe and inspectable but cannot promise identical outcomes. The API must return a plan and per-operation result, not a single success boolean.

## Rejected alternatives

Automatic re-run risks duplicate effects; file-only restore cannot resume agent execution.

