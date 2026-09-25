# ADR-M4-020: Cross-Platform Native Runtime Regression Gate

**Date:** 2026-09-26
**Status:** Proposed / remediation pending after native runs `36168364866` and `36173494807`

## Context

Windows is the live development host. M4-019 established the supported
Core-owned path and a passing Windows full regression. Native workflow runs
`36168364866` and `36173494807` reached both Ubuntu 24.04 and macOS 14 and
passed format/check/clippy, but the focused/full test matrices still exposed
fixture-lifetime failures. Existing historical native artifacts are not the
same as this M4-020 run: they use different workflows, commits, or test
scopes.

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
when they implement the same contract. Test fixture lifetime is part of the
cross-platform regression harness: a cold reopen must keep its temporary
project alive, and hosted macOS runs must use a physical temporary root when
the safety boundary rejects `/var` symlink ancestry. No production code or
schema change is justified by these test-harness failures.

## Consequences

Windows remains PASS. The Docker Desktop WSL2 Linux probe remains
supplementary diagnostic evidence only. Linux and macOS have real native FAIL
evidence from runs `36168364866` and `36173494807`; they must not be relabeled
PASS until the test-only fixture fix is pushed and the workflow reruns
cleanly. The isolated macOS focused ownership conflict was not
reproduced in that run's full suite and remains unclassified. Retain generated
artifacts, including exact toolchain, filesystem identity, commands, exit
codes, and cold-reopen results.
