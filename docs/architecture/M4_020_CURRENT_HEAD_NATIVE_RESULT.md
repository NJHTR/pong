# M4-020 Current-Head Native Runtime Result

**Run date:** 2026-10-04
**Repository HEAD:** `56a51eba4d5306a2244d7ed200a862a0bfb3c022`
**Fresh Windows source SHA:** `704d39c2f27e8399438936df18ef54eef80ba4b2`
**Branch:** `dev`

This is a current-head revalidation record. It does not rewrite the accepted
historical native run `36229863325`, which executed commit
`07f74b3a012c49b98b1dd6aaa7baa094658679c8`.

The fresh Windows artifact was captured before the evidence-only commit that
moved the repository from `704d39c` to `56a51eb`. That commit changed only this
result documentation and evidence files; no source, test, workflow, or
Protocol input changed. Ubuntu/macOS were still not rerun for either SHA.

## Checkpoint

| Area | Status | Basis |
| --- | --- | --- |
| Windows native | **PASS** | Fresh current-head run on Windows x86_64 |
| Ubuntu 24.04 native | **NOT_PROVEN** | No current-head Ubuntu runner is available in this environment |
| macOS 14 native | **NOT_PROVEN** | No current-head macOS runner is available in this environment |
| Cross-platform gate | **BLOCKED / ENVIRONMENT** | Current-head Linux/macOS native evidence is missing |
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

The retained run `36229863325` is valid evidence for the older commit
`07f74b3a012c49b98b1dd6aaa7baa094658679c8`. The current branch contains
subsequent commits, including changes after that run, so its Linux and macOS
results are not silently promoted to current-head results.

No Ubuntu 24.04 or macOS 14 runner was available for a fresh run at the current
HEAD. No Docker output was used as a substitute. Therefore the cross-platform
gate remains blocked by environment evidence availability, not by a discovered
Core or Protocol failure.

## Scope Boundaries

- Exploration E1/E2 were not changed or extended.
- E3 Candidate/Evaluation/Selection was not started.
- M4-021 provider interoperability was not touched.
- Protocol v1.0 and production code were not modified.
- `easyCode`, `D:\\bs\\seekwd`, and the pre-existing provider/Windows
  untracked files were not touched.

## Next Action

Run the existing workflow on Ubuntu 24.04 and macOS 14 for current HEAD
`704d39c2f27e8399438936df18ef54eef80ba4b2`. Do not claim the M4-020
cross-platform gate complete until both native jobs produce current-head
artifacts with the required command and exit-code records.
