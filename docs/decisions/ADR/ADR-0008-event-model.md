# ADR-0008: Append-Only Causal Events with Projections

- Status: Accepted
- Date: 2026-08-19

## Context

Agent activity must be auditable and reconstructable while status queries need fast materialized views.

## Problem

Should mutable tables or an event stream be authoritative?

## Options

1. Mutable status rows only.
2. Event stream only, rebuilt for every query.
3. Append-only events as facts plus idempotent projections and periodic snapshots.

## Decision

Choose option 3. Events have per-stream sequence, causation/correlation IDs, schema versions, and integrity metadata. Projections are rebuildable and never change the original facts.

## Consequences

Audit and recovery improve; schema evolution, compaction, and duplicate handling become explicit work.

## Rejected alternatives

Status-only storage loses history; event-only reads are too slow and operationally brittle.

