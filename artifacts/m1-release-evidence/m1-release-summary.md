# M1 Release Evidence Summary

Generated 2026-08-27 from the current `dev` working tree. This is an audit
bundle, not a release artifact.

## Decision

`M1 Release Gate: NOT PASSED`

The current-host core evidence is green for `PT-13` and `FI-10`, and Windows
stable/MSRV quality gates are green. Release acceptance remains blocked by
missing external evidence and governance decisions.

## Evidence disposition

| Area | Status | Release status | Reason |
| --- | --- | --- | --- |
| PT-13 projection/migration | PASS | BLOCKED | Current Windows host only; owner and cross-platform acceptance pending. |
| FI-10 projection fault schedule | PASS | BLOCKED | A-J and repeated-crash fixtures pass on current host; native/power-loss evidence absent. |
| Windows stable/MSRV gates | PASS | BLOCKED | Clean release tree and owner acceptance are absent. |
| Linux overlay / Docker-VM ext4 | PASS | BLOCKED | Executable evidence only; not native Linux acceptance. |
| Old v0.1 reader | BLOCKED | BLOCKED | Separately released binary, hash, and mutation probe absent. |
| Native Linux ext4 | FAIL | BLOCKED | Run `33085292318` produced a complete Ubuntu 24.04/ext4 artifact. Stable/MSRV fmt/check/clippy and focused suites passed, but both full tests exited `101` in artifact-consistency because the run consumed committed LF evidence while references still described prior Windows working-tree bytes. A fresh rerun after the byte-boundary correction is required. |
| Native macOS arm64 | FAIL | BLOCKED | Run `33085292318` produced a complete macOS 14 arm64 artifact. Stable/MSRV fmt/check/clippy and focused suites passed, but both full tests exited `101` in artifact-consistency for the same committed-byte mismatch. Filesystem metadata is `unknown`, not APFS evidence; a fresh rerun is required. |
| FI-01..FI-14 full matrix | BLOCKED | BLOCKED | Partial synthetic/host rows; external fault schedule and power-loss rows absent. |
| Performance / capacity | PASS | BLOCKED | Repeated measurements exist; ADR-0015 remains proposed. |
| Release owner | BLOCKED | BLOCKED | Unassigned; no sign-off or exception. |

## Artifact use

`m1-release-matrix.json` is the machine-readable normalized index. The
`build/`, `platform/`, `compatibility/`, `fault/`, `performance/`, and `logs/`
directories retain copied evidence or explicit absence notes. `SHA256SUMS`
covers every file in this bundle except itself. Source paths are preserved in
the artifact references manifest. The three `windows-stable-close-run-*`
performance files and logs are fresh measurement-only reruns from this
close-out; they do not accept ADR-0015.

Do not use this bundle to claim a released Core, public API, CLI, SDK, server,
or M2 readiness.
