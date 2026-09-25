# M4-020 Cross-Platform / Native Runtime Regression Gate

**Date:** 2026-09-24  
**Baseline:** `b71798d7b3be144f449deac6fa0ff39eca5fb234`  
**Preparation/evidence commit:** `cad20cd51979addba5811f28266994f514d1edd5`
**Status:** `BLOCKED / NATIVE REGRESSION FAILURES` after the first native
Ubuntu/macOS workflow run. Windows remains passing; remediation is prepared
for a rerun.

## Scope

M4-020 asks whether the already-frozen Pong contract is reproducible on native
Linux and macOS. It does not change External Agent Protocol v1.0, AgentControl,
Core ownership, Repository schema, HTTP semantics, or the unsupported Model A
boundary.

The contract under test is:

```text
Runtime clients
        -> Protocol / HTTP
        -> one exclusive Core owner
        -> Repository
```

The implementation may use platform-specific file locks, atomic rename
primitives, process termination, and cleanup. The durable semantics must not
change: Core ownership, second-Core rejection, lease/revision/CAS conflicts,
Operation idempotency, authorization, protocol lifecycle, and cold-reopen
recovery remain the same.

## Capability Inventory

| Capability | Current implementation boundary | M4-020 evidence |
| --- | --- | --- |
| OS and filesystem | Windows NT 10.0.26200.0, local workspace | Windows PASS from M4-019; Linux/macOS require native runners |
| Rust toolchain | rustc/Cargo 1.95.0 locally; crate MSRV 1.78 | Local Windows inventory captured; native workflow captures runner versions |
| Core owner lock | `fs2::FileExt`, shared/exclusive lock and process-local registry | Windows PASS; Linux/macOS NOT_PROVEN in this slice |
| Repository lock | shared handle for normal access, exclusive migration fence | Same contract; native execution required |
| Atomic file replacement | Windows `MoveFileExW`; Linux `renameat2`; macOS `renamex_np` | Source audit complete; native behavior NOT_PROVEN here |
| Process lifecycle | `std::process::Command`, child wait, platform-specific termination | Windows PASS; Linux/macOS NOT_PROVEN here |
| Temporary fixtures | `tempfile::tempdir` with per-test Repository/Workspace roots | Cross-platform test design present |
| SQLite | bundled rusqlite with WAL/sidecar scans and busy timeout | Windows PASS; Linux/macOS NOT_PROVEN here |
| Loopback HTTP | `tiny_http` server and bounded `TcpStream` client | Windows real-process PASS; Linux/macOS NOT_PROVEN here |
| Credential boundary | strict external file, opaque bearer, provider-neutral verifier | Windows real-process PASS; native parity NOT_PROVEN here |

## Required Native Matrix

The workflow at
`.github/workflows/m4-cross-platform-regression.yml` runs on
`ubuntu-24.04` and `macos-14`. Each runner captures platform identity and
executes:

1. `cargo fmt --all -- --check`
2. `cargo check --all-targets --locked`
3. `cargo clippy --all-targets --all-features --locked -- -D warnings`
4. focused Core ownership, lifecycle, concurrency, protocol, HTTP,
   authorization, reconnect, and real-process E2E tests
5. `cargo test --all --locked`

The focused matrix includes Core ownership, control-layer publication,
Operation ledger, Version/Workspace persistence and diff, materialization,
restore/rollback, real process startup, second-Core rejection, restart, cold
reopen, lease/revision/CAS behavior, HTTP authentication/authorization,
credential rotation, graceful shutdown, JSONL/HTTP semantic equivalence, and
failure isolation. The full suite remains the release gate; ignored contract
placeholders are not counted as passes.

## Current Results

| Platform | Result | Evidence |
| --- | --- | --- |
| Windows | `PASS` | M4-019 checkpoint and current repository baseline |
| Linux | `FAIL` | GitHub Actions run `36168364866`, Ubuntu 24.04 focused/full regression |
| macOS | `FAIL` | GitHub Actions run `36168364866`, macOS 14 focused/full regression |
| Protocol v1.0 | `PASS / unchanged` | No DTO or wire-shape change |
| HTTP transport | `PASS on Windows` / native parity `NOT_PROVEN` | Existing M4-015/M4-019 evidence |
| Authorization | `PASS on Windows` / native parity `NOT_PROVEN` | M4-018 evidence |
| Core ownership | `PASS on Windows` / native parity `NOT_PROVEN` | M4-019 evidence |
| Cold reopen and recovery | `PASS on Windows` / native parity `NOT_PROVEN` | Existing regression evidence |

The first real native run completed all three quality gates on both platforms,
then exposed two test/runner-boundary defects:

- Linux deleted the temporary project directory with the fixture before
  `d7_cold_reopen_after_restore` reopened the Repository. The same lifetime
  error surfaced as `NotFound` in four `agent_execution` tests.
- macOS hosted-runner temporary paths inherited a `/var` symlink ancestry,
  which the existing workspace safety boundary correctly rejected as a
  symlink/reparse path.

The remediation keeps the project and workspace `TempDir` values alive during
the Linux reopen and assigns macOS tests a physical `/private/tmp` `TMPDIR`.
These changes do not alter production code, Protocol v1.0, Core schema, or
durable semantics. The complete run record is retained in
[`m4-020-github-actions-run-36168364866.json`](../../artifacts/m4-development/m4-020-github-actions-run-36168364866.json)
and its companion log.

Historical M1 Linux/macOS artifacts are retained, but they do not close this
slice: they were produced by different workflows, commits, and matrices.
This document deliberately does not relabel them as M4-020 PASS.

## Environment Boundary

The current host is Windows only. Docker Desktop can provide a Linux userland,
but the 2026-09-24 Docker Desktop WSL2 probe did not pass the focused or full
regression and is not native Ubuntu evidence. It therefore does not promote
Linux to `PASS`. A real native workflow did execute on Ubuntu 24.04 and
macOS 14, but both jobs failed after the quality gates. M4-020 remains
`BLOCKED / NATIVE REGRESSION FAILURES` until a remediation rerun produces
complete artifacts with zero exits and the same semantic assertions.

The supplementary Docker result is retained in
[`m4-020-docker-linux-probe-2026-09-24.json`](../../artifacts/m4-development/m4-020-docker-linux-probe-2026-09-24.json)
and its companion log. It is diagnostic evidence only, not a native-platform
release row.

No production code was changed for this remediation. No MCP, SDK, TLS, public
Internet deployment, Model A support, or protocol redesign is included.
