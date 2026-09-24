# M4-020 Cross-Platform / Native Runtime Regression Gate

**Date:** 2026-09-24  
**Baseline:** `b71798d7b3be144f449deac6fa0ff39eca5fb234`  
**Preparation commit:** `36aba9e976065fada8148fddb302cf1554563c75`
**Status:** `BLOCKED / ENVIRONMENT` for Linux and macOS in this local
Windows-only session.

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
| Linux | `NOT_PROVEN` | No Linux runner was executed in this session |
| macOS | `NOT_PROVEN` | No macOS runner was executed in this session |
| Protocol v1.0 | `PASS / unchanged` | No DTO or wire-shape change |
| HTTP transport | `PASS on Windows` / native parity `NOT_PROVEN` | Existing M4-015/M4-019 evidence |
| Authorization | `PASS on Windows` / native parity `NOT_PROVEN` | M4-018 evidence |
| Core ownership | `PASS on Windows` / native parity `NOT_PROVEN` | M4-019 evidence |
| Cold reopen and recovery | `PASS on Windows` / native parity `NOT_PROVEN` | Existing regression evidence |

Historical M1 Linux/macOS artifacts are retained, but they do not close this
slice: they were produced by different workflows, commits, and matrices.
This document deliberately does not relabel them as M4-020 PASS.

## Environment Boundary

The current machine is Windows only. It can execute and verify the Windows
quality gates, but it cannot produce native Linux or macOS evidence. A
workflow file is executable preparation, not a platform result. M4-020 remains
`BLOCKED / ENVIRONMENT` until both native jobs produce complete artifacts
with zero exit codes and the same semantic assertions.

No production code was changed for this preparation slice. No MCP, SDK, TLS,
public Internet deployment, Model A support, or protocol redesign is included.
