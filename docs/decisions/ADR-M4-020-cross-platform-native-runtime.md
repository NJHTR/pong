# ADR-M4-020: Cross-Platform Native Runtime Regression Gate

**Date:** 2026-09-24
**Status:** Proposed / environment-blocked pending native Linux and macOS runs

## Context

Windows is the only live development host in this session. M4-019 established
the supported Core-owned path and a passing Windows full regression, while
Linux and macOS remained `NOT_PROVEN`. Existing historical native artifacts
are not the same as an M4-020 run: they use different workflows, commits, or
test scopes.

## Decision

Add a dedicated native workflow that runs the M4 contract matrix on Ubuntu and
macOS runners. Treat the workflow as a gate definition, not as evidence until
the jobs actually execute and upload complete records.

The workflow must keep the same contract across platforms:

- one exclusive Core owner and deterministic second-Core rejection;
- durable Agent, Task, Execution, Workspace, Operation, Snapshot, Version,
  Checkpoint, Handoff and Resume state after cold reopen;
- lease, revision, CAS, idempotency and conflict semantics;
- unchanged External Agent Protocol v1.0;
- real HTTP process authentication, authorization, reconnect, restart,
  credential rotation and graceful shutdown;
- failure errors retain their transport/protocol/authorization/domain layer;
  and
- direct multi-process Repository access remains Model A `NOT_SUPPORTED`.

Platform-specific locking, rename, signal and cleanup mechanisms are allowed
when they implement the same contract. No production code or schema change is
justified by the absence of a native runner.

## Consequences

Windows remains the only current PASS. Linux and macOS are explicit
`NOT_PROVEN` until native jobs run. This prevents CI configuration from being
mistaken for platform certification and keeps deployment limitations honest.
The next action is to run the workflow on both native platforms and retain
the generated artifacts, including exact toolchain, filesystem identity,
commands, exit codes, and cold-reopen results.
