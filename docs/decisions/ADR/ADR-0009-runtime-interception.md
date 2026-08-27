# ADR-0009: Layered Runtime Capture

- Status: Accepted
- Date: 2026-08-19

## Context

No generic hook sees every filesystem, shell, browser, network, and framework action. Strong claims require enforcement or framework cooperation.

## Problem

How can Pong provide implicit recording without claiming impossible completeness?

## Options

1. Filesystem watcher only.
2. Require every framework to call Pong explicitly.
3. Generic wrappers/watchers plus framework adapters, confidence labels, and reconciliation.

## Decision

Choose option 3. Capture layers include SDK/middleware, process and filesystem instrumentation, browser/HTTP/DB adapters, and framework lifecycle hooks. Each record declares method and confidence; explicit API can fill gaps.

## Consequences

Integration is broad and honest, but support matrices and crash/reconciliation tests are mandatory. Pong is not automatically a security sandbox.

## Rejected alternatives

Watcher-only misses intent and process calls; explicit-only violates the implicit runtime goal.

