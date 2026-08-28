# M1 Evidence

**Status: M1 Release Gate NOT PASSED.**

The auditable close-out bundle is
[`artifacts/m1-release-evidence/`](../../artifacts/m1-release-evidence/), with
the normalized matrix, build metadata, retained raw evidence, source
references, and `SHA256SUMS`. Its scoped `PASS` entries are evidence claims;
the release decision remains `NOT PASSED` until the blocked rows are supplied
and accepted.

## Current closure status

| Gate | Status | Evidence | Environment | Remaining |
| --- | --- | --- | --- | --- |
| PT-13 | PASS | `m1-pt13-fi10-summary.json` / raw log | Windows host, three reruns | Native-platform and owner acceptance |
| FI-10 | PASS | `m1-pt13-fi10-summary.json` / raw log | Windows host, A-J and repeated crash | Native filesystem/power-loss and owner acceptance |
| Old Reader | BLOCKED | Compatibility absence record | No released v0.1 binary or tag | Independent binary, hash, fixture, read/write probe |
| Native Linux | FAIL | GitHub Actions runs `33074865773`, `33080915116`, `33085292318`, and `33142438624`, artifact `m1-linux-native-evidence` | Ubuntu 24.04 / ext4 / Rust 1.98 + 1.78 | Run #4 still failed both full tests with the same committed `build-metadata.json` checksum drift; the release bundle checksum/reference boundary is now corrected and another rerun is required |
| Native macOS | FAIL | GitHub Actions runs `33074865773`, `33080915116`, `33085292318`, and `33142438624`, artifact `m1-macos-native-evidence` | macOS 14 arm64 / filesystem `unknown` / Rust 1.98 + 1.78 | Run #4 still failed both full tests with the same checksum drift; filesystem metadata is unknown and cannot be called APFS; rerun required |
| Git traceability | PASS (repository) / BLOCKED (release) | `build-metadata.json` | `dev`; bundle traceability is a pre-closeout snapshot; no release tag | Release-owner-approved release tag |

This is an evidence report, not a release claim. It separates executable
evidence from implementation that still lacks the platform, compatibility,
fault, or performance acceptance required by
[`M1_DURABLE_PRIMITIVES_GATE.md`](M1_DURABLE_PRIMITIVES_GATE.md).

The workflow definition was audited on 2026-08-27. The first manually
dispatched run (`33074865773`, commit
`1212930d5d4faab9ca6b66cb8745475ad9a6de46`) completed both matrix jobs but
failed the full quality gates. The retained artifacts are under
[`artifacts/m1-platform-runs/github-actions-run-33074865773/`](../../artifacts/m1-platform-runs/github-actions-run-33074865773/).
The downloaded ZIP SHA-256 values are Linux
`91694F6DFA456D475AE7F7FA385B3D72130329FAADD204C7DA1B48AC58687D44` and
macOS `AE75632EDD526E35D0D6E317E1EEF791F0773F698FF26BF01B001E6C547EC48F`.
The focused migration, recovery, compatibility, PT-13/FI-10, and cold-reopen
steps passed on both runners. Linux stable/MSRV full tests failed because
retained Windows evidence hashes drifted after checkout line-ending
normalization. macOS stable/MSRV full tests and clippy failed because the
host-resource fault test compiled platform-inapplicable helpers. These are
recorded failures, not native-platform passes. A second manually dispatched run
(`33080915116`, commit `8d28075d44f5458e866c94ee33b95b430f7959d6`) was then
retained. Linux reached the full stable/MSRV gates and focused suites, but both
full test commands again failed in `artifact_consistency` with the same retained
evidence hash drift. macOS only produced the Rust 1.78 build logs and focused
logs; its MSRV test failed while MSRV clippy exited zero, and the artifact omitted platform
metadata, the manifest, stable logs, and stable command exit records. The
downloaded ZIP SHA-256 values are Linux
`B3E457A9B936347E619C280B5DB5ACE06A9C7A7C206178193D85527BDF9E0B1E` and macOS
`01A1BC12E9DE8051F603EEDA3CC23A920419067F38503F38635856628083F16F`.
These are recorded failures, not native-platform passes. The workflow now prepares stable and
Rust 1.78 quality gates in independent target directories, pins test
repositories to a workspace-local temporary directory, records per-command
exit codes, runs the focused M1 and `cold_reopen` suites, captures the actual
runner and test-repository filesystems, and uploads `m1-linux-native-evidence` or
`m1-macos-native-evidence` with a structured metadata/manifest/checksum set.
The failed run is real native-platform execution, but it is not acceptable gate
evidence until the corrected commit completes successfully. See
[`M1_CI_EXECUTION_REQUIRED.md`](M1_CI_EXECUTION_REQUIRED.md) for the minimum
external handoff and rerun procedure.

A third manually dispatched run (`33085292318`, commit
`60913c0179731d9fed1a052f9a190c8fa4f5a56e`) produced complete Linux and
macOS artifacts. Stable and Rust 1.78 `fmt`, `check`, and clippy, all focused
M1 suites, and cold reopen returned zero on both runners. Both full test
commands nevertheless exited `101` in `artifact_consistency`: the run checked
out the committed LF blobs while the references still described the prior
Windows working-tree bytes (`build-metadata.json` and the PT-13/FI-10 raw-log
record). Run #3 is retained as complete failure evidence with Linux ZIP
SHA-256 `D8C255461BA1A64C712B55D79837F59B4B15E63A7D2A2341F4B86B54BCA57E0E`
and macOS ZIP SHA-256
`3C16C855381479C30B86BF6E3061818306F23DC55C7F9E11240178000A4A5692`.
The repository-wide `artifacts/** -text -diff` rule and explicit release-log
reference now make the committed evidence byte boundary deterministic; a fresh
native rerun is still required before either row can change from `FAIL`.

A fourth manually dispatched run (`33142438624`, commit `e96131d9bb6d3799055401841d0cb710e4f497ff`) produced complete Linux and macOS artifacts. All format, check, clippy, focused, and cold-reopen commands passed; only the two full test commands per platform exited `101`. The exact failure was `tests/artifact_consistency.rs:157`: the committed LF `build-metadata.json` blob was hashed as `5E1561C7C3E32AF94B36D271700A50C41371920B3FFE6DEF5E634E8F77B134A5`, while `SHA256SUMS` and `artifact-references.json` still recorded the former Windows CRLF hash `BF4AD3F6FA334833D420DDB954AC94EFBEB70450B632DCDEC3ACBFB8990B5DE1`. The ZIP SHA-256 values are Linux `DA903E4D7C6E11D3DD08BEE598883BE813E606305FA56ABA91D2154ACD0E6666` and macOS `501832097AB33640CFE062A98293CC4A36C7B83C8FCAF3918253D89F2716AD34`. The checksum and reference records are now rebound to the committed LF bytes, and a clean index export plus local artifact-consistency test pass; Run #4 remains retained failure evidence and does not promote M1.

## Reproduction

Windows (the current development host):

```powershell
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo +1.78.0 check --locked
cargo +1.78.0 test --locked
```

The Rust 1.78 commands use an independent target directory when the full
matrix is run:

```powershell
$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-msrv-178-final'
cargo +1.78.0 check --locked
cargo +1.78.0 test --locked

$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-msrv-178-clippy-final'
cargo +1.78.0 clippy --locked --all-targets -- -D warnings
```

Linux Rust 1.78 was run in the disposable pinned
`rust:1.78-slim-bookworm` container with `CARGO_TARGET_DIR=/root/pong-target`
(a Docker named volume, not the checked-in workspace). The container image has
rustup shims for `cargo-fmt` and `cargo-clippy`; the final run installed the
matching 1.78 components only inside the disposable container.

The final 2026-08-26 rerun used the pinned
`rust:1.78-slim-bookworm@sha256:0fea967628dc796a2b9d1d57ddb3af3b3f0a35b6c8c0e23690dbe0ceb71a2dc9`
image, installed `build-essential` only inside the disposable container, and
used the independent named target volume `pong-msrv178-slim-target`. Full
Linux `cargo fmt --all -- --check`, `cargo check --locked`, `cargo test
--locked`, and `cargo clippy --locked --all-targets -- -D warnings` passed.
The container rerun set `PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin`
and installed the matching `rustfmt` and `clippy` components before invoking
Cargo; this is required because the slim image does not expose those shims on
the default shell path.

The same pinned Rust 1.78 image was also run from an independent Docker named
volume backed by ext4 (`df -T` reports `/dev/sdf ext4`). The authoritative run
set `TMPDIR=/repo/tmp`, so test repositories and their WAL/CAS/staging files
were on the ext4 volume; source and target volumes were separate from the
checked-in workspace and existing build output. All four quality-gate commands
passed. The structured record and raw log are retained in
[`artifacts/m1-platform-runs/linux-ext4-rust-178-run-2-2026-08-26.json`](../../artifacts/m1-platform-runs/linux-ext4-rust-178-run-2-2026-08-26.json).
This is Docker-VM ext4 evidence, not native Linux host/VM ext4 acceptance.

The final Windows commands used fresh target directories:

```powershell
$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-stable-final'
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings

$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-msrv-178-final'
cargo +1.78.0 check --locked
cargo +1.78.0 test --locked

$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-msrv-178-clippy-final'
cargo +1.78.0 clippy --locked --all-targets -- -D warnings
```

All commands above exited zero, including the complete migration, recovery,
compatibility, property, Workspace/Snapshot, and Operation suites.

The continuation audit on 2026-08-26 repeated the Windows stable and Rust
1.78 quality gates in fresh `target/windows-stable-audit-*` and
`target/windows-msrv-178-audit-*` directories. Both complete test suites and
both clippy runs exited zero; the opt-in Windows FI-13 schedule was then run
separately with its raw output retained under
[`artifacts/m1-fault-runs/`](../../artifacts/m1-fault-runs/).

After the FI-01/FI-08 cold-reopen tests were added, the final continuation
rerun used fresh `target/windows-stable-fault-*` and
`target/windows-msrv-178-fault-*` directories. Windows stable and Rust 1.78
fmt/check/test/clippy all exited zero. The pinned Linux image was rerun with
the independent named target volume `pong-msrv178-slim-target-fault`; its
fmt/check/test/clippy gate also exited zero, including both new tests.

The 2026-08-27 Windows rerun used fresh independent target directories for
stable Rust 1.95.0 and the Rust 1.78.0 MSRV. `fmt --check`, `check --locked`,
the complete `test --locked` suite, and `clippy --all-targets -- -D warnings`
all exited zero for both toolchains. The rerun first exposed and then fixed a
Rust 1.78-only `needless_borrows_for_generic_args` lint in the evidence test;
the artifact-consistency test was rerun after the fix. This is Windows
evidence only; it does not replace the pinned Linux or external platform rows.

The final local close-out rerun on 2026-08-27 used the independent target
directories `target/windows-m1-close-stable-20260827` and
`target/windows-m1-close-msrv-178-20260827`. Direct execution (without a
wrapper that could mask a subcommand failure) recorded exit code `0` for each
of `fmt --all -- --check`, `check --locked`, `test --locked`, and
`clippy --locked --all-targets -- -D warnings` under both Rust 1.95.0 and
Rust 1.78.0. The dedicated `artifact_consistency` integration test also
passed once under each target. These are reproducibility records for the
Windows host only; they do not close the old-reader, native-platform,
cross-platform fault, projection, budget, or release-owner blockers.
The literal gate command set requested by the release checklist (`cargo
fmt --check`, `cargo check`, `cargo test`, and `cargo clippy -- -D warnings`,
with the `cargo +1.78.0` prefix for the MSRV run) was then executed against
the same target directories; every command again returned exit code `0`.

The Windows stable PT-14 normative command was run separately with
`PONG_PROPTEST_CASES=10000` and the independent target
`target/windows-m1-pt14-normative-20260827`:

```powershell
$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-m1-pt14-normative-20260827'
$env:PONG_PROPTEST_CASES = '10000'
cargo test --locked --test property_recovery_migration `
  pt14_migration_failpoints_are_old_or_verified_new_and_retry_idempotently `
  -- --nocapture
```

The harness executed 10,008 cases (nine failpoints, 1,112 cases each),
reported `1 passed; 0 failed`, and exited `0` after 1,376.66 seconds of test
time (1,400.947 seconds wall-clock). No host error or mixed-generation state
was observed. The retained record and raw log are
[`windows-stable-pt14-10000-rerun-2026-08-27.json`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json)
and [`windows-stable-pt14-10000-rerun-2026-08-27.log`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.log);
the raw log SHA-256 is
`5CA3901D442AA8C2F69EFC08AED925703526D4C5536CA5F34694B45AE8F90171`.

Real host-resource commands are documented in
[`tests/HOST_RESOURCE_FAULTS.md`](../../tests/HOST_RESOURCE_FAULTS.md). The
Linux command uses a fresh 64 MiB `tmpfs`; the Windows command uses a dedicated
NTFS scratch directory and restores its ACL before the final cold reopen.

The measurement probe is reproducible with:

```powershell
$env:PONG_PERF_OUTPUT = 'artifacts/m1-performance.json'
cargo test --locked --test performance_measurements -- --ignored --nocapture
```

The `m1-perf-0.3` record reports raw samples plus p50/p95/p99/max for
repository open, metadata intent/outcome, event append, and recovery. It also
records CAS throughput, eight independent migration samples, and
small/medium/large local snapshot scales. Three stable, three Rust 1.78
Windows, and three pinned Linux overlay runs from the 2026-08-26 audit are
retained under
[`artifacts/m1-performance-runs/`](../../artifacts/m1-performance-runs/).

## A. VERIFIED

The following claims have executable evidence in the current implementation:

| Area | Evidence and observed result |
| --- | --- |
| Windows baseline | Rust `1.95.0` `fmt`, `check`, full `test`, and `clippy -D warnings` pass in the fresh `target/windows-stable-final` directory. |
| Windows MSRV | Rust `1.78.0` `check` and full `test` pass in the fresh `target/windows-msrv-178-final` directory, and `clippy --all-targets -- -D warnings` passes in the independent `target/windows-msrv-178-clippy-final` directory; migration, process-kill, recovery, WAL, compatibility, Workspace/Snapshot, Operation, and property suites are included. |
| Linux MSRV | In pinned `rust:1.78-slim-bookworm` (with ephemeral `build-essential`, rustfmt, and clippy components), `fmt`, `check`, full `test`, and `clippy -D warnings` pass using the independent named target volume. |
| Linux Docker-VM ext4-backed volume | Rust `1.78.0` in the pinned image passes `fmt`, `check`, full `test`, and `clippy -D warnings` from separate source/target volumes with `TMPDIR` pinned to ext4; `df -T` identifies the repository and test temporary directory as `ext4`. Raw output and hashes are retained in [`artifacts/m1-platform-runs/`](../../artifacts/m1-platform-runs/). This remains executable evidence pending release-owner acceptance and does not claim native Linux ext4 support. |
| Native Linux run `33080915116` | GitHub Actions Ubuntu 24.04/ext4; [`download metadata`](../../artifacts/m1-platform-runs/github-actions-run-33080915116/download-metadata.json) and retained artifact | Stable and Rust 1.78 `fmt`, `check`, and clippy passed; focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed; both full `test` commands exited `101` in `artifact_consistency` because retained evidence hashes drifted after checkout. | Failure evidence only; bundle-wide `-text` fix is committed, and another rerun is required. |
| Native macOS run `33080915116` | GitHub Actions macOS 14 arm64; filesystem metadata unavailable; [`download metadata`](../../artifacts/m1-platform-runs/github-actions-run-33080915116/download-metadata.json) and retained artifact | Focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed; MSRV `test` exited `101` and MSRV clippy exited `0`; the artifact contains no platform metadata, manifest, stable logs, or stable exit records. | Incomplete failure evidence; capture/upload path must be fixed and rerun. |
| Native Linux run `33085292318` | GitHub Actions Ubuntu 24.04/ext4; [`download metadata`](../../artifacts/m1-platform-runs/github-actions-run-33085292318/download-metadata.json) and retained artifact | Complete artifact. Stable/MSRV `fmt`, `check`, and clippy plus focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed; both full `test` commands exited `101` in `artifact_consistency` because committed evidence bytes differed from the references used by the run. | Failure evidence only; byte-boundary fix is now in the worktree and another native rerun is required. |
| Native macOS run `33085292318` | GitHub Actions macOS 14 arm64; [`download metadata`](../../artifacts/m1-platform-runs/github-actions-run-33085292318/download-metadata.json) and retained artifact | Complete artifact. Stable/MSRV `fmt`, `check`, and clippy plus focused migration/recovery/compatibility/PT-13/FI-10/cold-reopen passed; both full `test` commands exited `101` in `artifact_consistency` for the same committed-byte mismatch. Filesystem metadata is `unknown`. | Failure evidence only; do not infer APFS support; another native rerun is required. |
| Migration atomicity | `tests/repository_migration.rs`, `tests/process_kill_migration.rs`, and `tests/property_recovery_migration.rs` exercise SQLite Online Backup, target verification, selector replacement, child termination, retry idempotency, and old-only/new-only visibility. No test adopts a merely existing partial generation. |
| Recovery and WAL | Child-process termination after intent/outcome boundaries, SQLite WAL-tail truncation, repeated recovery, and unknown-outcome preservation pass on Windows and Linux Rust 1.78. |
| CAS and security | CAS digest/immutability, quarantine, short-write and synthetic quota/permission paths pass. Generation identity, manifest identity, redaction profile, wrong-valid-database replacement, and full `.pong` byte scans fail closed. `foreign_valid_generation_database_cannot_replace_the_active_identity` additionally replaces the active file with a separately valid 0.2 database carrying a different generation/migration identity and observes `INTEGRITY_ERROR`. `artifact_consistency.rs` verifies retained raw-log hashes and FI-matrix artifact paths without changing acceptance status. |
| Property subsets | PT-01/02/03/04/05/06/07/08/11/12 each have a retained 10,000-case Windows stable run. PT-09 has a completed Windows stable rerun: 10,000/10,000 passed in 1,081.76 seconds, retained at [`windows-stable-pt09-10000-rerun-2026-08-26.json`](../../artifacts/m1-property-runs/windows-stable-pt09-10000-rerun-2026-08-26.json). PT-10 also has a completed Windows stable rerun: 10,000/10,000 passed in 1,325.25 seconds with exit code `0`, retained at [`windows-stable-pt10-10000-rerun-2026-08-26.json`](../../artifacts/m1-property-runs/windows-stable-pt10-10000-rerun-2026-08-26.json). PT-14 now has a completed Windows stable normative rerun: requested 10,000, harness executed 10,008 cases across 9 migration failpoints, `1 passed; 0 failed`, exit code `0`, with no observed host error or mixed-generation state; the structured record and raw log are [`JSON`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json) / [`raw log`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.log). Focused/concurrent diagnostic records remain supplemental, and normative runs on other accepted platform/filesystem rows are still absent. The earlier PT-09 no-result record [`windows-stable-pt09-10000-2026-08-26.json`](../../artifacts/m1-property-runs/windows-stable-pt09-10000-2026-08-26.json) and aggregate PT-10/PT-14 no-result record [`windows-stable-properties-10000-2026-08-26.json`](../../artifacts/m1-property-runs/windows-stable-properties-10000-2026-08-26.json) are retained as historical audit evidence. PT-13 migration semantics and FI-10 projection crash schedules have current-host evidence in `artifacts/m1-pt13-fi10-summary.json`; cross-platform runs and release-owner acceptance remain open. These are implementation/property evidence only, not release-owner acceptance of the full normative property gate. |
| Windows FI-13 | Real NTFS ACL revocation returned kernel error 5 and Pong `PERMISSION_DENIED`; committed ref/CAS data remained readable after ACL restoration and cold reopen. The retained stdout and structured record are in [`artifacts/m1-fault-runs/`](../../artifacts/m1-fault-runs/). |
| Linux FI-14 | Three retained pinned Rust 1.78 Docker `tmpfs` runs reached real Linux `errno=28` for CAS, metadata, and journal writes and returned Pong `RESOURCE_EXHAUSTED`; committed history, failed-write visibility, staging cleanup, and cold reopen assertions passed. Records: CAS [`JSON`](../../artifacts/m1-fault-runs/linux-fi14-cas-2026-08-26.json) / [`raw log`](../../artifacts/m1-fault-runs/linux-fi14-cas-2026-08-26.log), metadata [`JSON`](../../artifacts/m1-fault-runs/linux-fi14-metadata-2026-08-26.json) / [`raw log`](../../artifacts/m1-fault-runs/linux-fi14-metadata-2026-08-26.log), and journal [`JSON`](../../artifacts/m1-fault-runs/linux-fi14-journal-2026-08-26.json) / [`raw log`](../../artifacts/m1-fault-runs/linux-fi14-journal-2026-08-26.log). This remains partial platform evidence and is not Windows native disk-full coverage. |
| Performance measurement | `artifacts/m1-performance.json` (Rust 1.78), `artifacts/m1-performance-windows-stable.json` (Rust 1.95), and nine retained raw runs under `artifacts/m1-performance-runs/` record repeatable Windows and pinned Linux-overlay samples. `m1-perf-0.3` includes p50/p95/p99/max for repeated timing operations and migration; CAS and snapshot measurements remain workload-scoped evidence. They are measurement evidence only. |
| PT-13 / FI-10 projection evidence | `tests/pt13_fi10.rs` passes eight migration, generation-isolation, malformed/corrupt-state, interruption, A-J failpoint, repeated-crash, and golden-state tests in three repeated Windows runs; Rust 1.78 isolated check/test/clippy also pass. Structured evidence and raw log are retained in `artifacts/m1-pt13-fi10-summary.json` and `artifacts/m1-release-evidence/logs/m1-pt13-fi10-summary.log`. This is current-host implementation evidence, not cross-platform or release-owner acceptance. |

The PT-14 raw log was reconstructed from the captured command-output chunks in
the audit session rather than produced by a fresh `Tee-Object` rerun; the
structured record records this capture method explicitly. The test itself ran
once to completion with exit code `0`, and the retained log hash is verified by
the artifact-consistency test.

Projection status correction: the property row above predates the projection
core implementation. The envelope, cursor, handler, rebuild APIs, and the
current-host PT-13/FI-10 migration/crash fixture evidence now exist. External
platform and release-owner acceptance remain open.

The dedicated eight-test, three-rerun row above is authoritative for PT-13/FI-10
current-host evidence. Earlier audit wording is historical only; cross-platform
and release-owner acceptance remain open.

## B. IMPLEMENTED BUT INSUFFICIENT EVIDENCE

Projection evidence note: current-host executable PT-13/FI-10 evidence is
retained in `artifacts/m1-pt13-fi10-summary.json` and the matching raw log;
external platform and release-owner acceptance remain open.

These capabilities exist in code or focused tests, but the normative M1 gate
does not accept them as complete yet:

1. **Supported-platform cold-start matrix.** Evidence covers the Windows
   development filesystem and Linux container filesystems (`overlay`, a
   Docker-VM `ext4`-backed named volume, and the disposable `tmpfs`). There is
   no accepted matrix for every platform or filesystem Pong would claim to
   support, and native Linux host/VM ext4 remains unverified.
2. **Old-reader compatibility.** The selector identity makes a v0.1-shaped
   reader reject v0.2 before mutation, and the raw v0.1 fixture remains
   readable. This is not a separately released v0.1 binary compatibility run.
3. **Fault matrix completeness.** Synthetic failpoints now have file-backed
   cold-reopen assertions for FI-01 and FI-08, alongside selected real
   process-kill/resource tests. Every FI-01 through FI-14 schedule has not
   been independently executed on every declared platform/filesystem, and
   power-loss behavior is not directly testable here. The machine-readable
   [`artifacts/m1-fault-matrix.json`](../../artifacts/m1-fault-matrix.json)
   records each row's test names, observed disposition, and remaining gap;
   FI-13/FI-14 remain partial platform matrices. The three Linux FI-14
   raw/structured records are retained under
   [`artifacts/m1-fault-runs/`](../../artifacts/m1-fault-runs/), but they do
   not close the cross-platform fault matrix.
4. **Property coverage.** The event envelope/projection core now provides a
   versioned source envelope, project-local cursor, generation-bound projection
   rows, idempotent handler application, degraded unknown-event handling, and
   atomic rebuild. PT-13 and FI-10 now have the required current-host
   forward/backward migration fixtures and crash schedule; native power-loss,
   cross-platform, and release-owner acceptance remain open. PT-03/04/05/06
   are current-port properties, not a language-neutral multi-adapter matrix.
   PT-14 has a completed Windows stable normative rerun plus focused,
   bounded diagnostic and concurrent runs; its selector sync/journal
   publication access-denied classification is fixed for the exercised
   Windows path, while broader filesystem coverage is still missing. A
   projection-scope audit is now narrowed to migration/rebuild evidence. The
   targeted PT-13/FI-10 fixture suite is retained at
   [`artifacts/m1-pt13-fi10-summary.json`](../../artifacts/m1-pt13-fi10-summary.json)
   with three successful eight-test reruns and raw output in
   [`artifacts/m1-pt13-fi10-summary.log`](../../artifacts/m1-pt13-fi10-summary.log).
   This closes the current-host implementation evidence gap, but does not
   constitute release-owner acceptance or native power-loss/filesystem
   coverage.
   PT-09 and PT-10 now each have a completed Windows stable 10,000-case
   record; their earlier bounded/aggregate no-result records remain retained
   as historical audit evidence. The retained Windows PT-14 normative record
   covers the stable NTFS row only; other platform/filesystem rows and
   release-owner acceptance remain open.
5. **Performance and capacity.** The probe now records p50/p95/p99/max for
   repository open, metadata write, event append, recovery, and migration,
   plus CAS and three local snapshot scales. Snapshot timing is still
   single-sample within each probe run, and no accepted p95/p99 journal,
   recovery, migration, snapshot, object-throughput, or quota budget exists.
   The probe remains a measurement rather than a release threshold.
 6. **Public contract.** The crate currently exposes Rust symbols so integration
   tests can compile, but those symbols are unreleased and have no stable
   compatibility contract. No stable CLI, SDK, Runtime, Server, or framework
   adapter is part of M1 evidence. The public-symbol surface must be reviewed
   before a release package is published. Direct `MetadataStore`/`Cas`
    constructors are implementation/test entry points and do not carry the
    repository selector and generation-identity boundary enforced by
    `Repository::open`; callers must not treat them as a supported API.
 7. **Event envelope and projection contract.** The accepted event-model
    documents require event type, recorded time, causal links, a project-local
    sequence, redaction/capture metadata, and versioned projection state. The
   current `EventRecord`/`events` table only persists a per-stream sequence and
   omits those envelope fields; the additive `event_envelopes` table now carries
   the reconciled envelope. The implementation includes a projection schema,
   in-process handler registry, durable project-local cursor, idempotent apply,
   degraded status, and atomic rebuild bound to generation and redaction
   identity. This closes the design gap at the core layer. PT-13 and FI-10 now
   have executable current-host migration/interruption/rebuild evidence
   retained in the `m1-pt13-fi10` artifacts, while cross-platform and
   release-owner acceptance remain open. M1 remains **not passed**. The proposed
     [`ADR-0016`](../decisions/ADR/ADR-0016-projection-contract.md) records the
     generation-bound projection contract required for the evidence work;
     it is not an accepted M1 decision.

## C. BLOCKED BY ENVIRONMENT

The following evidence cannot be honestly produced in this workspace:

- a separately released historical Pong v0.1 executable and its old-reader
  mutation/open matrix;
- Windows-native disk-full/quota exhaustion without risking the host volume;
- power-loss testing, hardware/filesystem corruption beyond the exercised
  SQLite/CAS corruption cases, and a complete external-process kill schedule;
- an independent acceptance decision for performance/capacity budgets.

The first hosted native-platform execution is retained as a failed run. A
workflow definition or failed artifact is not a platform pass; the corrected
commit must complete the same commands and retain new logs, filesystem identity,
and hashes before either native row can be accepted.

These are recorded as unavailable evidence, not as passing assumptions.

The supplied workspace is a Git repository on branch `dev` with remote
`https://github.com/NJHTR/pong.git`. The bundle's traceability record points to
the pre-closeout execution snapshot; the current evidence update is deliberately
kept separate from those captured command results. There is no release tag or
owner-approved release commit. Evidence remains traceable by exact path,
command, toolchain, platform, test result, and retained artifact hash.

### External verification handoff

The following blockers require an external environment or a separately
released artifact. No owner is assigned yet, so these are handoff records and
not acceptance decisions. The owner must attach raw output, toolchain/lockfile
identity, and the cold-reopen result to the corresponding matrix row.

| Blocker | Required owner and environment | Rerun command / required handoff |
| --- | --- | --- |
| Historical v0.1 reader | Compatibility release owner (unassigned); isolated fixture plus the separately released v0.1 executable | Obtain the binary and documented read/mutation probe; record `Get-FileHash -Algorithm SHA256 <reader>` and the exact artifact invocation against the v0.2 selector fixture. No substitute current-source test is accepted. |
| Native Linux ext4 | Linux platform owner (unassigned); native ext4 host or VM | From the repository root, set an isolated `CARGO_TARGET_DIR` and run `cargo +1.78.0 fmt --all -- --check`, `cargo +1.78.0 check --locked`, `cargo +1.78.0 test --locked`, and `cargo +1.78.0 clippy --locked --all-targets -- -D warnings`; retain filesystem identification and a cold reopen. |
| macOS or another declared target | Platform owner (unassigned); target-native host/VM | Repeat the pinned-MSRV quality gate in a fresh target directory and attach the platform/filesystem row before declaring support. No target is declared by default. |
| Windows-native disk-full/quota | Windows storage owner (unassigned); disposable VM/volume or quota-controlled scratch filesystem | Run the applicable real-host FI-14 schedule from [`tests/HOST_RESOURCE_FAULTS.md`](../../tests/HOST_RESOURCE_FAULTS.md) without exhausting the development volume; retain raw OS error, Pong status, and cold-reopen inspection. |
| Power-loss, hardware/filesystem corruption, and full FI schedule | Reliability owner (unassigned); disposable VM or bare-metal harness with recoverable disk image | Execute every FI-* point in [`M1_DURABLE_PRIMITIVES_GATE.md`](M1_DURABLE_PRIMITIVES_GATE.md), including a cold restart after each interruption; attach per-case command, exit status, raw error, and inspection log. |
| Performance/capacity decision | Release owner (unassigned); every accepted platform/filesystem row | Run the probe three times per accepted row (`$env:PONG_PERF_OUTPUT='artifacts/m1-performance.json'; cargo test --locked --test performance_measurements -- --ignored --nocapture` in PowerShell, or the equivalent POSIX environment assignment), retain all raw samples, then record the ADR-0015 decision below. |

## Release-owner acceptance register

This is the authoritative sign-off record for this evidence package. Evidence
in section A is executable but does not become a release claim until a named
owner records a decision and date. The current placeholders are deliberately
unassigned.

| Artifact / decision | Owner | Status | Decision date | Retained record |
| --- | --- | --- | --- | --- |
| Platform and filesystem matrix | Unassigned | Pending; no row accepted | Not recorded | `M1_COMPATIBILITY_MATRIX.md` |
| Historical v0.1 reader compatibility | Unassigned | Blocked; binary/hash/probe missing | Not recorded | `M1_COMPATIBILITY_MATRIX.md` |
| FI-01 through FI-14 disposition | Unassigned | Pending; complete declared-platform evidence missing | Not recorded | [`artifacts/m1-fault-matrix.json`](../../artifacts/m1-fault-matrix.json) and fault artifacts |
| Property corpus and regression records | Unassigned | Pending; bounded subsets only | Not recorded | Section A and `proptest-regressions/` |
| ADR-0015 performance/capacity budget | Unassigned | Proposed; acceptance pending | Not recorded | [`ADR-0015`](../decisions/ADR/ADR-0015-m1-performance-capacity-budget.md) |

**No M1 artifact is accepted at this time.** Changing a row to `Accepted`
requires the named release owner to attach the retained record and an explicit
scope or exception; a green local test alone is insufficient.

## D. KNOWN RISKS

1. A repeated/concurrent Windows run previously observed intermittent
   `IO_ERROR` around PT-14 selector-directory-sync and journal-finalization
   boundaries. Windows raw access-denied error 5 could pass through generic
   I/O conversion during directory sync or repository-owned JSON publication.
   Those protected boundaries now use `from_protected_io`; the stable result is
   `PERMISSION_DENIED`, and three concurrent PT-14 reruns plus the stable/MSRV
   full suites pass. Selector uncertainty remains outcome-unknown and must
   never roll back the selector.
2. Windows transient sharing/access-denied retries are bounded to a two-second
   settling window. A host that holds a handle longer can still return a
   stable `PERMISSION_DENIED`/`IO_ERROR`; callers must retry the operation and
   re-run the cold-open invariant rather than infer selector state from the
   error alone.
3. The migration lock is understood by current Pong processes only. Older
   binaries must be closed before offline migration; the lock cannot stop an
   old writer that does not know it exists.
4. A generation manifest's metadata digest attests to the verified backup
   image at publication time. The active SQLite file is mutable afterward, so
   startup relies on SQLite integrity, profile, format, and in-database
   generation identity checks rather than a stale byte digest.
5. Host ACL, quota, antivirus, and directory-sync behavior remains
   filesystem-specific. A green synthetic failpoint does not widen the
   supported-platform claim.
6. The Rust crate currently exposes implementation modules and direct storage
   constructors to integration tests. They are not a stable Core contract and
   can bypass repository-scoped generation validation if used directly; the
   eventual public API must expose Repository-scoped ports and keep raw store
   constructors internal or test-only.
7. The retained Windows PT-14 high-load regression seeds demonstrate that a
   storage-heavy schedule can exercise sharing/access-denied timing at
   migration publication boundaries. The stable 10,000-case normative rerun
   now converges with no observed host error or mixed generation, but the
   result is Windows-only and does not replace native filesystem/fault rows.

## E. RELEASE ACCEPTANCE

**NOT PASSED.**

The gate remains blocked by the missing separately released old-reader matrix,
unaccepted supported-platform/cold-restart matrix, incomplete real fault
matrix, current-host-only FI-10/PT-13 projection migration evidence, and
absent accepted performance/capacity budgets. The historical
Windows PT-14 selector/journal publication error now has a reproduced root
cause, an error-mapping fix, and stable/MSRV/concurrent rerun evidence;
cross-platform directory-sync coverage is still not complete.
The current implementation is useful internal Rust evidence, but it is not a
released Pong Core or a stable public API. Existing M2/M3 slices are frozen
and do not authorize further phase work until M1 is accepted.

The next release-gate action is external review and evidence collection: attach
the separately released old reader, native platform rows, native disk-full and
power-loss-like schedules, and an accepted ADR-0015 budget, then record a named
release-owner decision. Until those artifacts are accepted, the roadmap must
not advance to public API, SDK, CLI, or framework adapters.
