# Next Task

**Current Phase:** Phase 1 - M1 release-scope decision closure

**Current Milestone:** M1 Durable local repository (release gate not passed)

**M1 release gate:** **Not passed.** M1 remains a prerequisite for any release
claim. No new M2/M3 feature work or public CLI, runtime, SDK, server, or UI
work is authorized until M1 is accepted. Existing internal slices remain frozen
evidence only.

**Current Task:** Execute the M1 release-scope decision package without
changing the settled storage architecture or production code. The proposed
scope is Windows x86_64/NTFS, Linux x86_64/ext4, and a GitHub-hosted macOS
runner whose filesystem remains `unknown`; this scope is not accepted until
the Release Owner records it. The release bundle at
`artifacts/m1-release-evidence/` contains executable platform, fault, property,
projection, and measurement evidence within its stated boundaries. Keep
FI-03, Linux FI-13, old-reader compatibility, property policy, ADR-0015, and
ADR-0016 as explicit proposals/pending decisions. Do not start M2 or add
Runtime, SDK, CLI, server, or framework-adapter work.

The minimal native-platform workflow is now defined at
`.github/workflows/m1-release-evidence.yml`. Runs `33074865773` through
`33142438624` are retained historical failures. Follow-up run `33145714975`
at commit `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` completed both native
jobs successfully; all 13 commands per platform returned zero and the
artifacts are now executable evidence `PASS`. This does not advance M1 until
the remaining release-blocking evidence and owner acceptance are closed.

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
release-log reference; the successful run is now retained and indexed. Follow
[`M1_CI_EXECUTION_REQUIRED.md`](../development/M1_CI_EXECUTION_REQUIRED.md)
for the external execution handoff.

**Blocked By:** Release Owner decisions are still required for the proposed
platform and fault scope, FI-03 deferral, Linux FI-13 applicability,
pre-M1/old-reader compatibility, the one-corpus property policy, ADR-0015,
ADR-0016, and the signed release-owner record. Traceability is also blocked by
the dirty working tree and absence of a candidate tag. The selected
`artifact-references.json` index has a packaging-hygiene caveat (it is not a
one-to-one checksum mirror and contains a stale self-entry); reconcile or
label it during candidate freeze without adding a new Gate condition. M2/M3
remain internal test-gated slices and do not waive M1.

**Next Action:** Stop technical M1 expansion and hand the prepared decision
package to the Release Owner. Review [`M1_RELEASE_SCOPE_DECISION.md`](../development/M1_RELEASE_SCOPE_DECISION.md),
[`M1_FAULT_SCOPE_DECISION.md`](../development/M1_FAULT_SCOPE_DECISION.md),
[`M1_PROPERTY_SCOPE_DECISION.md`](../development/M1_PROPERTY_SCOPE_DECISION.md),
[`M1_PERFORMANCE_SCOPE_DECISION.md`](../development/M1_PERFORMANCE_SCOPE_DECISION.md),
[`M1_PROJECTION_ACCEPTANCE.md`](../development/M1_PROJECTION_ACCEPTANCE.md),
and [`M1_OWNER_ACTION_LIST.md`](../development/M1_OWNER_ACTION_LIST.md).
No native/property suite is to be rerun in this scope-decision pass. The Owner
must either supply the missing external evidence or record bounded exclusions,
then accept/reject ADR-0015 and ADR-0016, freeze the final evidence snapshot,
and complete the sign-off. Do not create a tag or enter M2 from this task.

Run `33142438624` at commit `e96131d9bb6d3799055401841d0cb710e4f497ff` is
retained as the fourth native failure record. Both complete artifacts show
stable/MSRV fmt/check/clippy, focused M1 suites, and cold reopen passing, but
both full test commands exited `101` in `artifact_consistency`: the committed
LF `build-metadata.json` differed from stale CRLF hash/reference values. The
corrected committed digest is now recorded in the bundle; push and dispatch
again before accepting either native row.

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
required after the checksum/reference correction.

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
`6cb62fb455e92ab731a4bb5233856d10c1f1ce93`, remote `origin`); the regenerated
traceability record correctly reports the current evidence tree as dirty, with
no release tag or owner-approved release commit. The retained executable
evidence is bound to native workflow `33145714975` at commit `6cb62fb` with
exact commands, toolchain identity, test results, and artifact hashes. Native
follow-up run `33145714975` at commit `6cb62fb` completed
both jobs successfully with all 13 commands per platform returning `0`; its
complete artifacts are retained and native evidence rows are executable
`PASS`. M1 remains blocked by the old-reader, complete fault/property scope,
performance budget, and release-owner decisions.

**Definition of Done:** Every M1 MUST-PASS row has platform-specific executable
evidence or an explicitly accepted ADR disposition. The evidence report says
`PASS` only after the release owner accepts the matrix, old reader, fault, and
budget artifacts. Until then the authoritative status remains `NOT PASSED`.

## Session rule

Every future session starts here. If the next action changes, update this file
before changing implementation or roadmap scope.
