# ADR-0010: Isolated Writable Workspaces and Optimistic Refs

- Status: Accepted
- Date: 2026-08-19

## Context

Concurrent agents can touch the same path, branch, or head. Silent last-writer-wins would corrupt collaboration context.

## Problem

What concurrency policy provides useful parallelism and deterministic conflicts?

## Options

1. Allow shared writable workspaces with locks.
2. Serialize all project activity.
3. Default one mutable lease per workspace, independent workspaces/branches, and compare-and-swap commits with explicit merges.

## Decision

Choose option 3. Shared writable workspaces are opt-in and driver-dependent. Branch movement, leases, and event streams use optimistic expected-version checks; conflicts are surfaced.

## Consequences

Agents get safe parallelism at the cost of merge UX and lease management. File and resource-specific conflict policies remain necessary.

## Rejected alternatives

Global serialization destroys throughput; locks alone do not resolve stale branches or external side effects.

