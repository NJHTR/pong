# ADR-0003: Separate Version and Execution Graphs

- Status: Accepted
- Date: 2026-08-19

## Context

Git's commit tree is excellent for semantic file states, while agent activity is a branching, causal stream of operations, tasks, and checkpoints.

## Problem

Can one graph represent both without losing meaning?

## Options

1. Only a commit DAG.
2. Only an event/trajectory log.
3. A version DAG linked to an execution DAG by operation ranges and checkpoint/snapshot references.

## Decision

Choose option 3. Commits, branches, and snapshot roots form the version DAG. Tasks, sessions, operations, events, and checkpoints form the execution DAG. Links are immutable IDs and causal references.

## Consequences

Diff/log remain comprehensible while replay and provenance retain tool context. Projections and cross-graph queries add implementation work.

## Rejected alternatives

Flattening execution into commit messages loses timing, failed calls, and non-file effects; flattening versions into events makes semantic collaboration expensive.

