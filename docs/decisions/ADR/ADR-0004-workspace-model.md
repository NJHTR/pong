# ADR-0004: Logical Workspace with Driver

- Status: Accepted
- Date: 2026-08-19

## Context

Local paths, containers, remote machines, sandboxes, and future pods have different lifecycle and filesystem semantics.

## Problem

How can a workspace migrate without changing its identity or coupling Core to a host?

## Options

1. Store and expose a physical path as workspace identity.
2. Make every workspace a container.
3. Keep a logical workspace ID and bind it to a versioned driver/materialization.

## Decision

Choose option 3. Drivers advertise capabilities; migration records detach/attach and materialization facts. v0.1 defaults to a local driver.

## Consequences

The model supports local and remote implementations, but drivers must define capability gaps and consistency. Physical path leakage is avoided.

## Rejected alternatives

Paths are not portable; requiring containers excludes simple local development and does not solve external state.

