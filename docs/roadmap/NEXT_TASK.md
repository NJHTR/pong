# Next Task

**Current Phase:** Phase 1 - Durable primitives release audit

**Current Milestone:** M1 Durable local repository (release gate not passed)

**M1 release gate:** **Not passed.** M1 remains a prerequisite for any release
claim. No new M2/M3 feature work or public CLI, runtime, SDK, server, or UI
work is authorized until M1 is accepted. Existing internal slices remain frozen
evidence only.

**Current Task:** Close the M1 evidence package without changing the settled
storage architecture. The event envelope and generation-bound projection core
is implemented as an internal slice; PT-13 migration fixtures and FI-10
crash/rebuild evidence are retained for the current Windows host. The release
bundle at `artifacts/m1-release-evidence/` indexes the platform/MSRV runs,
fault disposition, property records, and measurement-only performance data.
The old-binary artifact, accepted platform scope, fault schedule, budget, and
owner sign-off are still missing. Keep all work below Runtime, SDK, CLI, server,
and framework adapters.

The minimal native-platform workflow is now defined at
`.github/workflows/m1-release-evidence.yml`; the first real run produced
artifacts but failed both native rows and therefore does not advance M1.

The workflow emits stable/MSRV logs, per-command exit codes, cold-reopen output,
workspace-local test-repository filesystem metadata, an artifact manifest, and
SHA256SUMS under the named native evidence artifacts. Run `33074865773` exposed
two real defects: Linux evidence hashes were invalidated by checkout line-ending
normalization, while macOS compiled a platform-inapplicable host-fault test.
Run `33080915116` was retained after the first correction: Linux still failed
both full tests in `artifact_consistency`, and macOS produced an incomplete
artifact while its MSRV test path failed (MSRV clippy exited zero). Both
failures are retained.
The earlier bundle-wide byte-preservation correction is pushed at `96a2f8c`;
workflow capture-boundary hardening is pushed at
`2cf136e99091d8d074d1a0e597f72f06bd2d52f7`. This close-out additionally
widens the Git byte boundary to all `artifacts/**` and fixes the PT-13/FI-10
release-log reference; commit and push these local changes before dispatching
the workflow again. Follow
[`M1_CI_EXECUTION_REQUIRED.md`](../development/M1_CI_EXECUTION_REQUIRED.md)
for the external execution handoff.

**Blocked By:** The evidence report still records missing separately released
old-reader compatibility, incomplete supported-platform/cold-restart coverage,
absent accepted budgets, and current-host-only FI-10/PT-13 projection
rebuild/migration acceptance described by proposed
[`ADR-0016`](../decisions/ADR/ADR-0016-projection-contract.md). The historical
Windows PT-14 selector/journal publication `IO_ERROR` has a reproduced root
cause, protected-boundary error-mapping fix, and stable/MSRV/concurrent rerun
evidence, but broader filesystem coverage remains pending. M2/M3 remain
internal test-gated slices and do not waive M1.

**Next Action:** Manually dispatch the native workflow again. Review
[`M1_EVIDENCE.md`](../development/M1_EVIDENCE.md)
sections A-E, [`M1_COMPATIBILITY_MATRIX.md`](../development/M1_COMPATIBILITY_MATRIX.md),
and `artifacts/m1-release-evidence/m1-release-summary.md`. PT-13/FI-10
current-host fixtures are complete. Obtain the external old-reader, native
Linux ext4/macOS, Windows disk-full, and full fault-schedule evidence listed in
the handoff table, or record an owner-approved bounded exception. The ADR-0015
performance decision and ADR-0016 projection contract remain proposed/pending
release-owner acceptance.

Run `33085292318` at commit `60913c0179731d9fed1a052f9a190c8fa4f5a56e` is
retained as the third native failure record. Both Linux and macOS artifacts
are complete; stable/MSRV fmt/check/clippy, focused M1 suites, and cold reopen
passed, but both full test commands exited `101` in `artifact_consistency`
because committed LF evidence bytes differed from the references used by that
run. The macOS runner reports filesystem metadata as `unknown`. The artifact
ZIP hashes are retained in the run download metadata. The repository-wide
`artifacts/** -text -diff` rule and explicit release-log reference now fix the
byte boundary; these changes are committed and pushed in
`258a0c9cccfe11e994a6032a106644b4dcf90804`. A fresh native dispatch remains
required.

The retained Windows stable property evidence includes PT-01/02/03/04/05/06/
07/08/11/12 at 10,000 cases each, PT-09 and PT-10 at 10,000 cases each, and the
PT-14 normative rerun at 10,008 executed cases across nine migration failpoints:
[`windows-stable-pt14-10000-rerun-2026-08-27.json`](../../artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json).
The PT-14 bounded concurrent diagnostics remain supplemental:
[`windows-stable-pt14-diagnostic-2026-08-27.json`](../../artifacts/m1-property-runs/windows-stable-pt14-diagnostic-2026-08-27.json).
Normative PT-14 runs on other accepted rows are still required. Historical
no-result records remain retained to show the audit boundary.

The continuation audit also added file-backed cold-reopen evidence for
synthetic FI-01/FI-08, the foreign-valid-generation SQLite replacement
regression, read-only artifact consistency checks, and a pinned Docker-VM ext4
run with `TMPDIR` on the ext4 volume. These strengthen local evidence but do
not satisfy the external platform, old-reader, fault, budget, or
release-owner requirements.

The 2026-08-27 close-out reran the complete Windows stable Rust 1.95.0 and
Rust 1.78.0 MSRV gates in independent target directories; every command
returned exit code `0`. Git traceability is available (`dev` at
`359abc306b554d592b532ebc182e543f97489043`, remote `origin`); the pre-closeout
working tree was clean, but no release tag or owner-approved release commit
exists. The retained executable evidence was captured before this closeout
commit; retain that snapshot relationship with the exact commands, toolchain
identity, test results, and artifact hashes for external review. The current
working tree contains the Run #3 retention and byte-boundary close-out in
pushed commit `258a0c9cccfe11e994a6032a106644b4dcf90804`; dispatch the workflow
against the current branch head before accepting either native row.

**Definition of Done:** Every M1 MUST-PASS row has platform-specific executable
evidence or an explicitly accepted ADR disposition. The evidence report says
`PASS` only after the release owner accepts the matrix, old reader, fault, and
budget artifacts. Until then the authoritative status remains `NOT PASSED`.

## Session rule

Every future session starts here. If the next action changes, update this file
before changing implementation or roadmap scope.
