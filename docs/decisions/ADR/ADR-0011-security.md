# ADR-0011: Pong as Recorder plus Policy Enforcement Boundary

- Status: Accepted
- Date: 2026-08-19

## Context

Pong observes sensitive files, shells, networks, databases, and credentials. A recorder cannot retroactively prevent damage, but unconstrained capture is unsafe.

## Problem

Is Pong merely an audit log or also a security boundary?

## Options

1. Recorder only.
2. Full sandbox/security product.
3. Recorder with explicit policy enforcement where it controls the execution boundary, and clear limitations elsewhere.

## Decision

Choose option 3. Pong enforces capabilities, redaction, approval, and workspace isolation through supported drivers/adapters; it does not claim to contain an uncooperative host process. Secrets never enter ordinary objects.

## Consequences

Integrations must declare trust level and bypass paths. Security tests and audit evidence are release gates.

## Rejected alternatives

Recorder-only is unsafe for controlled runtimes; a full sandbox would expand scope beyond versioning infrastructure.

