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
Both failures are retained. The portability correction is pushed at
`359abc306b554d592b532ebc182e543f97489043`, with evidence metadata bound in
`2310c822a6f5d3f942067e0ec7a322369f03df8a`; a manual rerun is now required.
Follow
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

**Next Action:** Review [`M1_EVIDENCE.md`](../development/M1_EVIDENCE.md)
sections A-E, [`M1_COMPATIBILITY_MATRIX.md`](../development/M1_COMPATIBILITY_MATRIX.md),
and `artifacts/m1-release-evidence/m1-release-summary.md`. PT-13/FI-10
current-host fixtures are complete. Obtain the external old-reader, native
Linux ext4/macOS, Windows disk-full, and full fault-schedule evidence listed in
the handoff table, or record an owner-approved bounded exception. The ADR-0015
performance decision and ADR-0016 projection contract remain proposed/pending
release-owner acceptance.

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
`359abc306b554d592b532ebc182e543f97489043`, remote `origin`); the working tree
is clean, but no release tag or owner-approved release commit exists. The
retained executable evidence was captured before this closeout commit; retain
that snapshot relationship with the exact commands, toolchain identity, test
results, and artifact hashes for external review.

**Definition of Done:** Every M1 MUST-PASS row has platform-specific executable
evidence or an explicitly accepted ADR disposition. The evidence report says
`PASS` only after the release owner accepts the matrix, old reader, fault, and
budget artifacts. Until then the authoritative status remains `NOT PASSED`.

## Session rule

Every future session starts here. If the next action changes, update this file
before changing implementation or roadmap scope.
