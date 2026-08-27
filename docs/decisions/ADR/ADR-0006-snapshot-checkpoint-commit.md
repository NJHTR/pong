# ADR-0006: Distinct Snapshot, Checkpoint, and Commit

- Status: Accepted
- Date: 2026-08-19

## Context

A filesystem state, a resumable agent state, and a collaboration version have different durability and semantic requirements.

## Problem

Can one object safely serve all three roles?

## Options

1. One universal snapshot object.
2. Git commits only.
3. Immutable Snapshot plus separate Commit and Checkpoint wrappers with explicit links.

## Decision

Choose option 3. Snapshot is a state root; Commit is a semantic version node; Checkpoint is a recovery contract with adapter state and replay policy. They may share content but never imply one another.

## Consequences

Users must choose the correct command and API. Storage and UI need clear affordances; recovery claims can be honest.

## Rejected alternatives

One object encourages false rollback/resume guarantees and conflates collaboration with execution.

