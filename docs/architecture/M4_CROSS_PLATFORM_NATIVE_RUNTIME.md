# M4-020 Cross-Platform / Native Runtime Regression Gate

**Date:** 2026-09-26
**Baseline:** `b71798d7b3be144f449deac6fa0ff39eca5fb234`  
**Preparation/evidence commit:** `cad20cd51979addba5811f28266994f514d1edd5`
**Status:** `BLOCKED / NATIVE REGRESSION FAILURES` after native workflow runs
`36168364866`, `36173494807`, `36185384714`, `36192502871`, and
`36227546466`. Windows remains passing; the next test-only fixture remediation
is prepared for another rerun.

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
| Linux | `FAIL` | GitHub Actions run `36192502871`, Ubuntu 24.04 focused/full regression |
| macOS | `FAIL` | GitHub Actions run `36192502871`, macOS 14 focused/full regression |
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

The first remediation keeps the project and workspace `TempDir` values alive
during the Linux reopen and assigns macOS tests a physical `/private/tmp`
`TMPDIR`. A second fixture-lifetime defect was found in rollback R9 reopen
tests and has a test-only fix in the current worktree. These changes do not
alter production code, Protocol v1.0, Core schema, or durable semantics. Run
records are retained in
[`m4-020-github-actions-run-36168364866.json`](../../artifacts/m4-development/m4-020-github-actions-run-36168364866.json)
and its companion log.

## Second Native Run

Run `36173494807` executed commit `a6bfc1eb38ab5875caa6244d737d1500286254c6`
on Ubuntu 24.04 and macOS 14. Format, check, and clippy passed on both
platforms. The four M7 materialization reopen tests still failed on both
platforms because the run commit did not yet contain the pending fixture
lifetime fix. The macOS focused matrix also recorded one isolated Core
ownership conflict; the same ownership test passed in that run's full
regression, so it remains an unclassified native observation rather than a
production fix target.

The immutable run record and artifact hashes are retained in
[`m4-020-github-actions-run-36173494807.json`](../../artifacts/m4-development/m4-020-github-actions-run-36173494807.json)
and its companion log. The current worktree validates that M7 materialization
fixture fix on Windows, but Linux and macOS remain `FAIL` pending native rerun.

## Fourth Native Run

Run `36185384714` executed commit `f342694ca11537da368fb9c380fc3cf3a506cd58`
on Ubuntu 24.04 and macOS 14. Format, check, and clippy passed on both
platforms. The previous M7 materialization reopen failures no longer
appeared. Both focused and full matrices instead failed at the same four
`cross_workspace_rollback` R9 reopen tests: the tests dropped
`CrossWorkspaceRollbackFixture` before reopening the Repository, deleting the
temporary project root. The native runner records show
`NotFound("project root does not exist")`.

The supplied Linux and macOS ZIP artifacts are identified by their SHA-256
hashes in
[`m4-020-github-actions-run-36185384714.json`](../../artifacts/m4-development/m4-020-github-actions-run-36185384714.json);
the corresponding run summary is retained in its companion log. On Windows,
the current test-only fix passes `cross_workspace_rollback` (47/47), the
focused M4 matrix, and `cargo test --all --locked`. These local results do
not promote native Linux or macOS to PASS.

## Fifth Native Run

Run `36192502871` executed commit
`a8f13f04c972c7371f96ada0644048230c84ffc3` on Ubuntu 24.04 and macOS 14.
Format, check, and clippy passed on both platforms. The rollback R9 and M7
materialization reopen failures no longer appeared. Both focused and full
matrices instead failed at:

- `cross_workspace_source::x21_cold_reopen`, which dropped
  `TwoWorkspaceFixture` before reopening the Repository and deleted the
  temporary project root.
- `http_hardening::process_credential_reload_rotates_and_revokes_without_touching_core_state`,
  whose Unix test helper wrote a credential file with broad permissions. The
  production loader correctly rejected that file before readiness, and the
  test then observed EOF while parsing an empty readiness line.

The supplied Linux and macOS ZIP artifacts and hashes are retained in
[`m4-020-github-actions-run-36192502871.json`](../../artifacts/m4-development/m4-020-github-actions-run-36192502871.json)
and its companion log. The current Windows worktree fixes both harness issues:
`cross_workspace_source` passes 30/30 and `http_hardening` passes 10/10, while
the focused M4 matrix and full `cargo test --all --locked` also pass. These
local results do not promote native Linux or macOS to PASS.

Historical M1 Linux/macOS artifacts are retained, but they do not close this
slice: they were produced by different workflows, commits, and matrices.
This document deliberately does not relabel them as M4-020 PASS.

## Sixth Native Run

Run `36227546466` executed commit
`a163057acf6e09d3abb5d23f65b1e1bf63563d93` on Ubuntu 24.04 and macOS 14.
Format, check, and clippy passed on both platforms. The M7 materialization,
rollback R9, source cold-reopen, and `http_hardening` credential fixtures no
longer failed. Both focused and full matrices instead failed at the one
remaining process-startup test:
`http_remote_transport::production_http_process_owns_core_authenticates_and_shuts_down_cleanly`.

The child `pong-agent-http` exited before emitting its readiness JSON because
this separate test wrote `credentials.json` with default Unix permissions.
The production credential loader correctly rejected group/other-readable
credentials, while the test helper discarded child stderr and reported only
`EOF while parsing a value`. This is a test-harness setup and diagnostics
defect, not an HTTP, Core, Protocol, or ownership semantic failure.

The current Windows worktree fixes that fixture by setting mode `0600` on Unix
and reports child status/stderr when readiness is absent. After the fix,
Windows `http_remote_transport` passes 16/16, the M4 focused matrix passes,
and `cargo test --all --locked` exits 0. These local results do not promote
native Linux or macOS to PASS.

The immutable run record and downloaded artifact hashes are retained in
[`m4-020-github-actions-run-36227546466.json`](../../artifacts/m4-development/m4-020-github-actions-run-36227546466.json)
and its companion log.

## Environment Boundary

The current host is Windows only. Docker Desktop can provide a Linux userland,
but the 2026-09-24 Docker Desktop WSL2 probe did not pass the focused or full
regression and is not native Ubuntu evidence. It therefore does not promote
Linux to `PASS`. Real native workflows executed on Ubuntu 24.04 and macOS 14,
but both jobs in run `36227546466` failed after the quality gates. M4-020
remains `BLOCKED / NATIVE REGRESSION FAILURES` until a remediation rerun
produces complete artifacts with zero exits and the same semantic assertions.

The supplementary Docker result is retained in
[`m4-020-docker-linux-probe-2026-09-24.json`](../../artifacts/m4-development/m4-020-docker-linux-probe-2026-09-24.json)
and its companion log. It is diagnostic evidence only, not a native-platform
release row.

No production code was changed for this remediation. No MCP, SDK, TLS, public
Internet deployment, Model A support, or protocol redesign is included.
