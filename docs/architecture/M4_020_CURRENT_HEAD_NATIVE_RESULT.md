# M4-020 Current-Head Native Runtime Result

**Run date:** 2026-10-04
**Repository HEAD:** `3fba86aa056ed7a028e3e8fe0735621399f76977`
**Fresh Windows source SHA:** `704d39c2f27e8399438936df18ef54eef80ba4b2`
**Branch:** `dev`

This is a current-head revalidation record. The current source evidence is Run
`37180210921`, which executed commit `3fba86a`. The Windows artifact was
captured earlier at `704d39c` and remains valid because the intervening commits
changed only evidence records. This record does not rewrite the accepted
historical native run `36229863325`, which executed commit
`07f74b3a012c49b98b1dd6aaa7baa094658679c8`.

The evidence-only commits after the Windows capture changed no source, test,
workflow, or Protocol input.

## Checkpoint

| Area | Status | Basis |
| --- | --- | --- |
| Windows native | **PASS** | Fresh current-head run on Windows x86_64 |
| Ubuntu 24.04 native | **PASS** | Run `37180210921`, current source `3fba86a` |
| macOS 14 native | **FAIL** | Run `37180210921`, one full-regression Core-owner conflict |
| Cross-platform gate | **BLOCKED / NATIVE REGRESSION FAILURE** | macOS full regression is not green |
| Protocol v1.0 | **UNCHANGED** | No Protocol change in this slice; protocol matrix passed |
| Production code | **NO CHANGE** | This slice only ran gates and added evidence |
| Workflow | **PRESENT** | Existing workflow declares `ubuntu-24.04` and `macos-14` jobs |
| Evidence | **PARTIAL** | Fresh Windows artifact plus retained historical native artifacts |
| Docker | **SUPPLEMENTARY ONLY** | Existing Docker probe is not native Ubuntu evidence |

## Windows Current-Head Run

Runner and toolchain:

- OS: Windows, AMD64, `x86_64-pc-windows-msvc`
- Rust: `rustc 1.95.0`
- Cargo: `cargo 1.95.0`
- Commit: `704d39c2f27e8399438936df18ef54eef80ba4b2`

All commands exited with code `0`:

- `cargo fmt --all -- --check`
- `cargo check --all-targets --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- M4 focused matrix from `.github/workflows/m4-cross-platform-regression.yml`
- `cargo test --all --locked`

The focused matrix covered Core ownership, lifecycle, lease/revision/CAS,
Operation recovery, checkpoint/handoff/rollback, Version/Workspace persistence,
reconnect/restart, HTTP/JSONL transport, authorization, and real process E2E.
Ignored contract placeholders remain ignored according to their existing
contract status and are not counted as passing implementations.

Evidence:

- Structured record:
  [m4-020-windows-current-head-2026-10-04.json](../../artifacts/m4-development/m4-020-windows-current-head-2026-10-04.json)
- Raw combined stdout/stderr:
  [m4-020-windows-current-head-2026-10-04.log](../../artifacts/m4-development/m4-020-windows-current-head-2026-10-04.log)

## Native Runner Boundary

### Requested `56a51eb` Native Dispatch

Status: **BLOCKED / ENVIRONMENT**. The existing workflow is dispatchable only
through GitHub Actions, but this environment has no authenticated `gh` CLI or
GitHub token. The public workflow page is accessible only as an anonymous
viewer and presents `Sign in` instead of a dispatch control. No new workflow
run, runner output, or artifact was produced for `56a51eb`.

This is an environment/access blocker, not a test failure. No Docker, WSL,
local simulation, or historical artifact is counted as Ubuntu 24.04 or macOS
14 evidence for `56a51eb`.

### Supplied Run #8 Analysis

The supplied archives are real native artifacts from workflow run
`37143141686`, but both jobs checked out `bd828fea75bf56fb02ca5a7898c84899289e7602`
(`bd828fe`), not the requested `56a51eb`. They therefore cannot be credited as
current-`56a51eb` evidence.

Archive hashes:

- Linux: `5C67725D15D3334C8F6A1A80965C4958E97B85075EE200C823EA046212977A76`
- macOS: `F91C235C0B246AD2B7E24B0AC3F48B78C41616B0AF7669151DDA868A74593214`

On both Ubuntu 24.04 and macOS 14, the actual run produced:

- fmt: exit `0`
- check: exit `0`
- clippy: exit `0`
- M4 focused matrix: exit `0`
- full regression: exit `101`

The full-regression failure was identical on both platforms. The first
failure was `tests/artifact_consistency.rs`, which rejected the committed
Windows evidence record because its `raw_log` was absolute and lacked the
required `raw_log_sha256`. This was an evidence-package schema defect, not a
Core, Protocol, or native runtime failure. The record has now been corrected to
use a repository-relative log path and the actual SHA-256. Local
`artifact_consistency` (2/2) and a subsequent full `cargo test --all --locked`
pass confirm the correction.

During the first local full rerun, two `remote_failure_semantics` assertions
also observed an empty diagnostic deque under concurrent scheduling. The suite
passed on immediate rerun and on 20 repeated runs; no production change was
made. This remains a test-observability timing observation, not evidence of a
native platform failure.

### Supplied Run #9 Analysis

Run `37180210921` executed the current branch head
`3fba86aa056ed7a028e3e8fe0735621399f76977` on native Ubuntu 24.04 and macOS
14, so it is current-source evidence.

Linux passed fmt, check, clippy, the focused matrix, and the full regression,
all with exit code `0`.

macOS passed fmt, check, clippy, and the focused matrix, but its full
regression exited `101` in one test:

`http_remote_transport::malformed_version_method_path_media_and_body_limit_are_transport_safe`

The failure occurred at `tests/http_remote_transport.rs:436` while opening a
fresh temporary repository as Core owner:

`Conflict("repository already has active access; Core ownership requires exclusive startup")`

The same test passed in the macOS focused matrix from the same workflow run.
This is classified as a test-fixture / Core-ownership lifecycle observation
under full-regression scheduling, not yet as a production Core defect. It is
not caused by the Node.js deprecation warning or the macOS capacity notice.
The failure requires isolated and controlled-parallelism reproduction before
any production change is considered.

Run #9 evidence is retained in
[`m4-020-github-actions-run-37180210921.json`](../../artifacts/m4-development/m4-020-github-actions-run-37180210921.json).

The retained run `36229863325` is valid evidence for the older commit
`07f74b3a012c49b98b1dd6aaa7baa094658679c8`. The current branch contains
subsequent commits, including changes after that run, so its Linux and macOS
results are not silently promoted to current-head results.

Run #9 supplies current-head Ubuntu evidence and current-head macOS failure
evidence. No Docker output was used as a substitute. The cross-platform gate
remains blocked by the macOS native full-regression failure.

## Scope Boundaries

- Exploration E1/E2 were not changed or extended.
- E3 Candidate/Evaluation/Selection was not started.
- M4-021 provider interoperability was not touched.
- Protocol v1.0 and production code were not modified.
- `easyCode`, `D:\\bs\\seekwd`, and the pre-existing provider/Windows
  untracked files were not touched.

## Next Action

Reproduce the macOS Core-owner conflict from Run #9 in isolation and with
controlled test parallelism. Do not claim the M4-020 cross-platform gate
complete until macOS full regression is green.
