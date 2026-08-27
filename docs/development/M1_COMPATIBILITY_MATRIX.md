# M1 Platform and Compatibility Matrix

**Status: evidence package in progress; no release support claim is made.**

The current close-out bundle is indexed at
[`artifacts/m1-release-evidence/m1-release-matrix.json`](../../artifacts/m1-release-evidence/m1-release-matrix.json)
with copied raw evidence, source references, and checksums in
[`artifacts/m1-release-evidence/`](../../artifacts/m1-release-evidence/). The
bundle does not change the acceptance status below: executable evidence may be
`PASS` for a scoped host while the release row remains `BLOCKED` pending owner
acceptance.

The external native-runner job is defined at
[`../../.github/workflows/m1-release-evidence.yml`](../../.github/workflows/m1-release-evidence.yml)
for Ubuntu 24.04 and macOS 14. Run `33074865773` completed both jobs and
produced retained artifacts, but both jobs failed the full quality gates. A
second dispatch, run `33080915116` at commit
`8d28075d44f5458e866c94ee33b95b430f7959d6`, was also retained: Linux again
failed both full tests in `artifact_consistency`, while macOS failed the MSRV
test path (MSRV clippy exited zero) and produced an incomplete artifact. The focused M1 suites
passed where captured. A third dispatch, run `33085292318` at commit
`60913c0179731d9fed1a052f9a190c8fa4f5a56e`, produced complete Linux and
macOS artifacts. Both platforms passed stable/MSRV fmt/check/clippy and all
focused suites, but both full test commands exited `101` in
`artifact_consistency` because the run observed committed LF evidence bytes
while the references still described the prior Windows working-tree bytes.
The macOS platform metadata reports filesystem `unknown`. Native rows
therefore remain `FAIL` pending a rerun after the byte-boundary correction,
not `PASS` or `BLOCKED`.

This matrix is the checklist for M1 acceptance. A row is `PASS` only when the
named executable artifact, platform, filesystem, toolchain, and cold-restart
inspection are retained. A source-level fixture or a direct SQL query is not a
substitute for a separately released old Pong binary.

## Platform and filesystem matrix

| Target | Filesystem | Toolchain | Cold check/test | Migration/process kill | Host resource faults | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Windows x86_64 MSVC | NTFS | Rust 1.78.0 | `cargo +1.78.0 check --locked`; `cargo +1.78.0 test --locked`; `cargo +1.78.0 clippy --locked --all-targets -- -D warnings`; isolated `target/windows-msrv-178-final` and `target/windows-msrv-178-clippy-final` | `repository_migration`, `process_kill_migration`, PT-14 focused/concurrent runs; stable-only PT-14 normative artifact is not an MSRV run | FI-13 ACL pass; native disk-full unavailable | Evidence present; release acceptance pending |
| Windows x86_64 MSVC | NTFS | stable 1.95.0 | `fmt`, `check`, full `test`, `clippy -D warnings`, isolated `target/windows-stable-final` | migration/recovery suites pass; PT-14 normative 10,008-case rerun plus focused/concurrent diagnostics pass; record [`windows-stable-pt14-10000-rerun-2026-08-27.json`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json) | FI-13 ACL pass; native disk-full unavailable | Evidence present; release acceptance pending |
| Linux x86_64 GNU | Docker overlay filesystem | Rust 1.78.0 | pinned `rust:1.78-slim-bookworm`, full `fmt/check/test/clippy -D warnings`, named volume `pong-msrv178-slim-target` mounted at `/root/pong-target` | migration/process-kill/recovery/WAL/property/Workspace/Snapshot/Operation suites pass | not the quota run | Executable container evidence only |
| Linux x86_64 GNU | disposable 64 MiB `tmpfs` | Rust 1.78.0 | focused host-fault test build | not applicable | FI-14 CAS/metadata/journal `ENOSPC` pass | Host-fault evidence only |
| Linux x86_64 GNU | Docker named volume backed by ext4 | Rust 1.78.0 | pinned image, fresh source volume, independent target volume, `TMPDIR` on ext4; fmt/check/test/clippy pass | migration/process-kill/recovery/WAL/property/Workspace/Snapshot/Operation suites pass | not a quota run | Evidence present; release acceptance pending; [`artifacts/m1-platform-runs/linux-ext4-rust-178-run-2-2026-08-26.json`](../../artifacts/m1-platform-runs/linux-ext4-rust-178-run-2-2026-08-26.json) |
| Linux x86_64 GNU | native ext4 | Rust 1.78.0 | run `33074865773`: focused cold-reopen passed; full test failed in artifact-consistency | focused migration/recovery/PT-13/FI-10 passed | not exercised by this workflow | **FAIL**; rerun after evidence line-ending fix |
| macOS arm64 | APFS | stable 1.98.0 + Rust 1.78.0 | run `33074865773`: focused cold-reopen passed; full test/clippy failed to compile host-resource test | focused migration/recovery/PT-13/FI-10 passed | not exercised by this workflow | **FAIL**; rerun after platform cfg fix |
| Linux x86_64 GNU | native ext4 | stable 1.98.0 + Rust 1.78.0 | run `33080915116`: focused cold-reopen passed; stable/MSRV fmt/check/clippy passed; both full tests failed in artifact-consistency with retained-evidence hash drift | focused migration/recovery/compatibility/PT-13/FI-10 passed | not exercised by this workflow | **FAIL**; bundle-wide `-text` fix is committed, rerun required |
| macOS arm64 | unknown | Rust 1.78.0 captured; stable capture absent | run `33080915116`: focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed; MSRV test exited `101`, MSRV clippy exited `0`; platform metadata, manifest, and stable logs are absent | focused suites passed where captured | not exercised by this workflow | **FAIL**; incomplete artifact, capture/upload fix and rerun required |
| Linux x86_64 GNU | native ext4 | stable 1.98.0 + Rust 1.78.0 | run `33085292318`: complete artifact; stable/MSRV fmt/check/clippy and focused suites passed, but both full tests exited `101` in artifact-consistency | focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed | not exercised by this workflow | **FAIL**; committed-byte mismatch evidence retained, rerun after byte-boundary correction required |
| macOS arm64 | unknown | stable 1.98.0 + Rust 1.78.0 | run `33085292318`: complete artifact; stable/MSRV fmt/check/clippy and focused suites passed, but both full tests exited `101` in artifact-consistency | focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed | not exercised by this workflow | **FAIL**; filesystem identity is unknown, committed-byte mismatch evidence retained, rerun required |

The final Linux overlay rerun used image digest
`sha256:0fea967628dc796a2b9d1d57ddb3af3b3f0a35b6c8c0e23690dbe0ceb71a2dc9` and
installed `build-essential`, rustfmt, and clippy only inside the disposable
container. All four Linux quality-gate commands passed. The current candidate
release scope must remain limited to rows with accepted
evidence. The table intentionally does not silently convert Docker overlay,
Docker-VM ext4, or `tmpfs` behavior into a claim for all Linux filesystems.

An additional authoritative Rust 1.78 run copied the source into the
independent `pong-linux-ext4-audit-repo` Docker named volume and set
`TMPDIR=/repo/tmp`. `df -T` identified both the repository and temporary
directory as `ext4`; the quality-gate output is retained under
[`artifacts/m1-platform-runs/`](../../artifacts/m1-platform-runs/). This is
stronger filesystem evidence than the bind-mounted overlay run, but it is
still not a native Linux host/VM ext4 acceptance row.

The post-FI-01/FI-08 continuation used the same pinned image and a separate
named target volume, `pong-msrv178-slim-target-fault`. Its full Linux
fmt/check/test/clippy run passed; this is refreshed executable evidence, not a
new platform/filesystem acceptance decision.

The retained dependency lock identity for the current package is
`Cargo.lock` SHA-256
`108E191A65F726519CF76791A080058887B4CF1EDBBF9D428BBF5553774DC2DE`.
The Windows rows retain `rustc 1.78.0 (9b00956e5 2024-04-29)` and
`rustc 1.95.0 (59807616e 2026-04-14)` output in the corresponding command
logs and performance records. These identities support auditability; they do
not constitute release-owner acceptance.

## Release-owner acceptance register

This register records the release decision separately from executable evidence.
An `Evidence present` row is not an accepted support claim until the release
owner records a decision here. `Unassigned`, `Pending`, or `Rejected` keeps the
corresponding row outside the release scope.

| Decision item | Owner | Decision | Date / retained record |
| --- | --- | --- | --- |
| Supported platform/filesystem scope | Unassigned | Pending; no platform row accepted | Not recorded |
| Historical v0.1 reader artifact and mutation probe | Unassigned | Pending; binary, SHA-256, and results missing | Not recorded |
| FI-01 through FI-14 host-fault disposition | Unassigned | Pending; complete declared-platform matrix missing | [`artifacts/m1-fault-matrix.json`](../../artifacts/m1-fault-matrix.json) |
| Property corpus and regression-seed acceptance | Unassigned | Pending; current runs are bounded evidence only | Not recorded |
| ADR-0015 performance/capacity budget | Unassigned | Proposed; release-owner acceptance pending | See ADR-0015 acceptance record |

**Current release scope: none accepted.** This register is intentionally
unassigned until a release owner reviews the retained artifacts and records a
named decision, date, and exception (if any). It must not be edited to
`Accepted` merely because a local command is green.

## Old-reader binary matrix

| Reader artifact | Source repository | v0.2 selector/generation open | Mutation before/after selector | Artifact identity | Status |
| --- | --- | --- | --- | --- | --- |
| Historical Pong v0.1 reader | v0.1 `.pong/metadata.sqlite` | not run | not run | no separately released binary/hash supplied | Missing, release-blocking |
| Current Rust reader in v0.1 compatibility tests | raw v0.1 SQLite fixture | not an old binary | direct SQL/current library only | current source tree | Supplementary only |
| Current Rust reader | v0.1 legacy repository | explicit offline migration then cold reopen | active generation writes only | current source tree | Pass for current-reader migration behavior |
| Current Rust reader | v0.2 selector with damaged/mismatched target | fail-closed before healthy open | no mutation | current source tree | Pass for integrity boundary |

The selector marker contract demonstrates the intended v0.1 behavior: a reader
that only accepts marker version 1/repository format 0.1 must return
`UNSUPPORTED` before opening or mutating the legacy path. It does **not** prove
the behavior of a separately released historical executable. M1 cannot close
until that executable, its build hash, and its read/mutation results are
attached to this matrix.

The executables under the current `target/**` directories are Rust test
harnesses, not reader releases: invoking their `--version` probe returns the
harness `Unrecognized option` error. They are intentionally excluded from the
old-reader artifact row.

## Required retained artifacts

For every future `PASS` row retain:

- platform and filesystem identification;
- `rustc --version`, dependency lock hash, and target directory;
- exact command and exit status;
- repository fixture or generated seed;
- process-kill/failpoint point and cold-reopen result;
- raw stable Pong error code for failures; and
- old-binary path, version, SHA-256, and mutation probe output where applicable.

Until all required rows are accepted by the release owner, the authoritative
M1 state remains `NOT PASSED` in `M1_EVIDENCE.md`.
