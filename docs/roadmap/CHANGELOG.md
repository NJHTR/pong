# Changelog

## Unreleased - M1 evidence close-out (2026-08-27)

- Retained GitHub Actions native evidence run `33085292318` for Linux and macOS; both artifacts are complete but remain `FAIL` because full tests detected committed evidence byte/hash drift.
- Extended artifact consistency coverage to the third native failure run and fixed the evidence byte boundary with `artifacts/** -text -diff` plus an explicit PT-13/FI-10 release-log reference.
- Rebuilt the release bundle references and `SHA256SUMS`; M1 remains `NOT PASSED` and a fresh native rerun is required.

## Unreleased - Phase 0 (2026-08-19)

- Established Pong scope as agent execution versioning and coordination infrastructure.
- Added vision, architecture, protocol, concept, security, reliability, compatibility, development, research, governance, ADR, and roadmap documentation.
- Recorded local-first storage, logical workspaces, extensible operation envelopes, event projections, two DAGs, policy-gated replay, and explicit framework boundaries.
- Added the M1 durable-primitives gate, ordered PoC execution plan, and experimental reports for crash ordering, capture confidence, secret redaction, and two-agent lease/merge.
- Accepted ADR-0013 for Rust Core plus bundled `rusqlite`; added the internal CAS/metadata/event scaffold and language-neutral M1 fixture schema.
- No public CLI, runtime, SDK, server, or released Core implementation added; the M1 scaffold remains gate-blocked.

## Unreleased - Phase 1 (2026-08-19)

- Added the internal Rust CAS and SQLite metadata/event primitives with bundled `rusqlite`, canonical object identity, WAL `FULL` durability, ref compare-and-swap, idempotency, and immutable event checks.
- Added redaction profile compatibility, pre-persistence redaction for metadata/events/journal fields, fail-closed scans of SQLite sidecars and CAS trees, secret rejection before object publication, and secret-safe debug output.
- Added the `.pong` repository layout and atomic `repository.json` startup marker with format/schema/storage-driver validation.
- Added recovery classification for orphan intents (`unknown`) during repository startup, idempotent journal phase transitions, restart/WAL/CAS boundary tests, and duplicate intent/event regression tests.
- Added instance-scoped CAS and metadata failpoints, cold-start recovery injection, child-process termination tests, SQLite WAL-tail truncation evidence, and a repository-wide fail-closed byte scan covering unknown `.pong` files.
- Added the cross-file repository-generation migration contract: SQLite Online Backup, generation manifests, offline repository locking, atomic `repository.json` selector replacement, selector-journal recovery, safe path validation, and old/new-only cold-restart tests.
- Added deterministic PT-01 through PT-12 and PT-14 property coverage. The
  retained 10,000-case artifact covers Windows PT-01/02/03/04/05/06/07/08/11/12;
  PT-09/10/14 have focused/default-case coverage but their storage-heavy
  10,000-case continuation was interrupted before producing a result.
- Added opt-in real-host FI-13 Windows ACL evidence and FI-14 Linux `tmpfs` ENOSPC evidence for CAS, metadata, and journal writes; all reported stable Pong permission/resource statuses without losing prior committed data.
- Added `artifacts/m1-performance.json` with metadata, recovery, CAS, and repository-migration measurements. It is measurement evidence, not an accepted performance budget.
- The M1 gate remains open: the supported-platform declaration, separately released old-reader matrix, accepted performance/capacity budgets, and complete fault matrix remain required. The historical Windows PT-14 selector/journal publication classification issue is fixed for the exercised paths, but broader filesystem coverage still needs acceptance.

## Unreleased - Phase 2 (2026-08-25)

- Added internal logical workspace, environment, and epoch-lease metadata with revision/lease guarded workspace updates and same-project environment binding.
- Added allowlisted deterministic environment facts for OS, architecture, family, and safe variables; sensitive keys, host identity/path keys, and obvious secret-shaped values are excluded before persistence.
- Added a bounded local filesystem workspace driver with logical identity, checked locators outside `.pong`, canonical portable tree manifests, immutable file blobs in CAS, symlink/reparse rejection, and file-count/per-file-size limits.
- Added materialization into a new temporary directory followed by verified file writes, directory synchronization, and rename. Existing destinations are never overwritten.
- Added integration coverage for lease/revision conflicts, cold-reopen lease takeover, repeated canonical snapshots, file-count/size limits, secret-file rejection without head advancement, materialization, same-project environment binding, manager-controlled head advancement, Unix symlink rejection, and sensitive environment allowlists.
- Added M2 fault-injection evidence for CAS publication, workspace-head commit boundaries, materialization writes/syncs/rename, parent-directory outcome-unknown publication, stable permission/resource errors, and explicit orphan cleanup after cold reopen.
- Added a raw v0.1 SQLite compatibility fixture proving migration sources are opened read-only without additive M2 DDL, unsupported formats fail before schema mutation, and target generations initialize additive workspace tables.
- Added the M2 workspace/snapshot acceptance gate. This is an unreleased internal slice: crash reconciliation, complete lifecycle operations, schema-evolution evidence, property/fault/performance matrices, and non-local drivers remain incomplete, and M1 is still not passed.

## Unreleased - Phase 3 (2026-08-25)

- Added the first internal M3 operation-ledger slice: versioned envelopes,
  durable start/terminal lifecycle rows, operation event streams, explicit
  effect classes, typed references, same-project identity bindings, redaction,
  request/operation idempotency, and before/after-commit cold-reopen evidence.
  Runtime interception and public APIs remain out of scope.

## Unreleased - M1 native platform closeout (2026-08-27)

- Retained real GitHub Actions run `33074865773` and both native evidence
  artifacts. Ubuntu 24.04 completed focused M1 evidence but failed full tests
  when Windows evidence hashes drifted after checkout line-ending
  normalization; macOS 14 completed focused M1 evidence but failed full tests
  and clippy because the host-resource fault test compiled unused
  Linux/Windows-only helpers. M1 remains not passed.
- Marked captured evidence logs as byte-exact Git `-text` files and constrained
  `tests/host_resource_faults.rs` to Linux/Windows targets. Corrected Windows
  stable/MSRV gates and artifact-consistency checks pass locally; a new native
  CI run is required before platform rows can be accepted.

- Audited and hardened `.github/workflows/m1-release-evidence.yml` for the
  native-platform closeout. The workflow now runs stable and Rust 1.78 gates
  in independent target directories, records exact command exit codes,
  pins test repositories to a workspace-local temporary directory, captures
  native filesystem identity and run metadata, executes focused cold-reopen
  evidence,
  and uploads deterministic Linux/macOS artifact names with structured
  metadata, manifest, and SHA256SUMS. The first real run `33074865773` is
  retained as a failure record; native rows remain unaccepted pending rerun.
- Added `docs/development/M1_CI_EXECUTION_REQUIRED.md` with the minimum
  commit/push/Actions/download handoff. No CI result, release tag, or owner
  acceptance is inferred from the local workflow definition.
- Published the M1 evidence workflow to `origin/dev` at
  `ca6323673cc87be30d377f3b0915f9061c2a038b`. A subsequent run exposed
  cross-platform evidence normalization and macOS target-cfg defects; the
  corrective commit and manual dispatch remain required.

## Unreleased - M1 native rerun handoff (2026-08-27)

- Fixed byte-exact evidence-log handling, constrained host-resource fault
  helpers to applicable targets, retained the failed native run `33074865773`,
  and corrected the release bundle's nested `SHA256SUMS` coverage.
- Pushed portability fix `359abc306b554d592b532ebc182e543f97489043` and metadata
  follow-up `2310c822a6f5d3f942067e0ec7a322369f03df8a` to `origin/dev`.
- Native Linux and macOS remain `FAIL` for the historical run; a new manual
  workflow dispatch is required before either row can be reconsidered. M1
  remains `NOT PASSED`.

## Unreleased - M1 evidence audit (2026-08-26)

- Assembled the M1 release evidence bundle under
  `artifacts/m1-release-evidence/`. It contains a normalized
  `PASS`/`FAIL`/`BLOCKED`/`NOT_APPLICABLE` matrix, Windows stable/MSRV build
  logs with explicit exit codes, copied platform, fault, performance,
  property, and PT-13/FI-10 records, source-path references, and `SHA256SUMS`.
  PT-13 and FI-10 remain `PASS` for the current Windows-host evidence scope,
  while release acceptance remains `BLOCKED`.
- Corrected stale audit wording: this checkout does contain Git metadata
  (`dev` at `2da6cf1037acf054b33b23b07c27388be1807690` with `origin`), but the
  working tree is dirty and has no accepted release tag or release commit.
- M1 remains **not passed**. The separately released v0.1 reader, native
  Linux/macOS rows, Windows native disk-full, real power-loss/full fault
  schedule, accepted ADR-0015 budget, and named release owner are still
  required.
- Added the minimal `.github/workflows/m1-release-evidence.yml` workflow for
  external native Linux (`ubuntu-24.04`) and macOS (`macos-14`) evidence. It
  captures platform identity, runs the Rust 1.78 quality gates plus
  projection/migration/recovery/compatibility tests, and uploads raw logs with
  hashes. No runner has executed in this workspace, so both native rows remain
  `BLOCKED`.

- Closed the current-host PT-13/FI-10 implementation evidence gap without
  changing the settled storage architecture. Added generation A-to-B
  projection migration, generation-isolation assertions, migration interruption
  checks, ProjectionFailpoint A-J crash/reopen schedules, repeated-crash
  recovery, and clean-replay golden-state comparisons. Three targeted reruns
  are retained in `artifacts/m1-pt13-fi10-summary.json` and
  `artifacts/m1-pt13-fi10-summary.log`; M1 remains **not passed** because
  external platform, old-reader, real fault, budget, and owner acceptance
  evidence is still required.

- Implemented the internal generation-bound event envelope and projection
  slice described by ADR-0016. Added additive `event_envelopes`, projection
  state/cursor tables, deterministic handler registration, idempotent apply,
  degraded unknown-event handling, atomic rebuild, legacy-event backfill, and
  schema-boundary validation. PT-13 migration fixtures, FI-10 crash schedules,
  and release-owner acceptance remain pending; M1 remains **not passed**.

- Reworked `docs/development/M1_EVIDENCE.md` into explicit A-E sections for
  passed evidence, implemented-but-insufficient evidence, unavailable host
  checks, known risks, and the release-gate decision.
- Re-ran the Rust 1.78 migration/recovery/compatibility/property suites in an
  independent Windows target directory and in a disposable Linux container;
  Linux full `cargo check` and `cargo test` passed.
- Re-ran real Windows NTFS FI-13 ACL revocation and real Linux 64 MiB `tmpfs`
  FI-14 CAS/metadata/journal exhaustion. The observed domain statuses were
  `PERMISSION_DENIED` and `RESOURCE_EXHAUSTED`, respectively.
- Kept M1 explicitly **not passed**: no separately released old-reader binary
  matrix, accepted platform/capacity budget, complete host fault matrix, or
  accepted supported-filesystem matrix exists yet. `NEXT_TASK.md` now points
  back to this release audit.
- Added the auditable [`M1_COMPATIBILITY_MATRIX.md`](../development/M1_COMPATIBILITY_MATRIX.md)
  and proposed [`ADR-0015`](../decisions/ADR/ADR-0015-m1-performance-capacity-budget.md);
  both remain pending release-owner acceptance.
- Expanded the measurement probe to include repository open, event append, and
  small/medium/large local snapshot scales. The new measurements remain
  evidence, not an automatic release threshold.
- Reproduced the concurrent Windows PT-14 selector-directory-sync failure,
  fixed the protected-directory raw access-denied mapping to
  `PERMISSION_DENIED`, and verified three concurrent PT-14 reruns. Broader
  filesystem coverage remains part of the M1 matrix.
- Extended the protected I/O mapping through repository-owned marker/journal
  atomic publication, closing the remaining Windows PT-14 journal-finalization
  `IO_ERROR` path without changing selector atomicity or retry semantics.
- Extended the bounded Windows transient sharing/access-denied retry window to
  80 attempts at 25 ms, and added Linux 1.78 clippy-driven portability fixes
  for path arguments. The complete Windows stable/MSRV and Linux quality gates
  pass after these changes.
- Re-ran the final Windows 1.95.0 quality gate, Windows Rust 1.78 isolated
  `check/test`, and pinned Linux Rust 1.78 full `fmt/check/test/clippy` after
  installing matching components in the disposable image; all exited zero.
  M1 remains **not passed** pending release-owner acceptance of the
  compatibility, fault, platform, and performance/capacity evidence.
- Refreshed `artifacts/m1-performance.json` with Rust 1.78 and
  `artifacts/m1-performance-windows-stable.json` with Rust 1.95.0 after the
  final code changes; these remain measurement evidence rather than accepted
  release budgets.
- Added explicit release-owner acceptance registers to the M1 compatibility
  matrix, evidence report, and ADR-0015, plus external-environment handoff
  commands for unavailable platform, old-reader, fault, and performance
  evidence. All decisions remain unassigned/pending and M1 remains **not
  passed**; the Linux final target-volume record is now consistent across the
  evidence package.
- Extended the measurement-only probe to `m1-perf-0.2` with p99 summaries for
  repeated timing operations, regenerated the Rust 1.78 and stable baseline
  records, and retained three raw runs per Windows toolchain under
  `artifacts/m1-performance-runs/`. These runs do not constitute ADR-0015
  acceptance or widen the supported-platform claim.
- Added a fresh isolated `target/windows-msrv-178-clippy-final` run after
  installing the Rust 1.78 clippy component; MSRV clippy now passes with
  `-D warnings` alongside the stable and Linux quality gates.
- Extended the performance probe to `m1-perf-0.3` with eight independent
  migration samples and p50/p95/p99/max summaries. The resulting artifacts
  remain measurement evidence only and do not accept ADR-0015.
- Added three pinned Linux overlay performance runs to the raw-run manifest;
  native ext4 and other filesystem claims remain explicitly unverified.
- Added `artifacts/m1-fault-matrix.json`, an auditable FI-01 through FI-14
  disposition index that records exact tests, observed statuses, and external
  gaps without promoting incomplete rows to pass.
- Re-ran the Windows NTFS FI-13 ACL schedule in a dedicated scratch directory
  and retained raw stdout plus a structured record with kernel error 5,
  `PERMISSION_DENIED`, ACL restoration, and cold-reopen results. Added the
  existing ref-CAS commit-boundary test to the FI-09 machine-matrix index.
  This remains partial host evidence; M1 is still **not passed**.
- Ran the Windows stable property corpus with `PONG_PROPTEST_CASES=10000`.
  PT-01/02/03/04/05/06/07/08/11/12 completed (10,000 cases each); the
  storage-heavy PT-09/10/14 continuation was interrupted before any result
  and is recorded as such. The raw log and case/seed manifest are retained
  under `artifacts/m1-property-runs/`; no incomplete run is counted as a pass.
- Added file-backed cold-reopen assertions for synthetic FI-01 and FI-08:
  pre-commit intent failure remains pre-state after reopen, and CAS directory
  sync failure reopens a complete immutable object with an idempotent retry.
  The fault matrix and harness docs retain the synthetic/native evidence
  boundary; M1 remains **not passed**.
- Re-ran the complete Windows stable, Windows Rust 1.78, and pinned Linux Rust
  1.78 quality gates after the FI-01/FI-08 additions in fresh target
  directories/volume `pong-msrv178-slim-target-fault`; all fmt/check/test/
  clippy commands exited zero. This confirms build portability but does not
  satisfy the external M1 release-owner, old-reader, filesystem, or fault
  matrix requirements.
- Audited the FI-10/PT-13 projection gap against ADR-0008 and the current
  metadata/event store. No projection schema/version, handler,
  cursor/checkpoint, or rebuild API exists; the matrix keeps FI-10
  `not_implemented` and M1 remains **not passed** rather than accepting an
  unrelated event-list or repository-migration test as projection evidence.
- Added a retained pinned-Rust-1.78 Linux run on an independent Docker named
  volume identified by `df -T` as ext4, with `TMPDIR` pinned to that volume so
  test repositories use the exercised filesystem. The complete
  fmt/check/test/clippy gate passed; this is Docker-VM filesystem evidence only
  and does not close the native ext4, old-reader, host fault, or release-owner
  acceptance gaps.
- Corrected the M1 evidence wording so the retained 10,000-case claim is
  limited to PT-01/02/03/04/05/06/07/08/11/12; PT-09/10/14 remain explicitly
  no-result for that long run. Marked the crate `publish = false` while its
  integration-test-visible Rust symbols remain unreleased implementation
  surface; direct metadata/CAS constructors are not a supported public API.
- Added a structured no-result record for the bounded Windows PT-09 10,000-case
  run, including the interruption boundary, observed exit code, and raw-log
  SHA-256. No result is inferred from the manual stop.
- Retained three independent pinned Linux Rust 1.78 `tmpfs` FI-14 records for
  CAS, metadata, and journal exhaustion, each with raw `ENOSPC`, Pong
  `RESOURCE_EXHAUSTED`, cold-reopen assertions, and raw-log hashes. These are
  partial platform evidence only; M1 remains **not passed**.
- Added a repository-migration regression proving that a separately valid
  generation-format SQLite database with a foreign generation/migration
  identity cannot replace the active generation and bypass selector binding.
- Added a read-only M1 artifact-consistency test that verifies retained
  raw-log SHA-256 values and fault-matrix artifact paths; it is evidence
  hygiene only and leaves the release gate not passed.
- Completed the Windows stable PT-09 recovery property rerun with 10,000/10,000
  cases passing in 1,081.76 seconds; retained the structured record and raw
  log hash. PT-10 and PT-14 remain separate incomplete normative obligations,
  and M1 remains **not passed**.
- Completed the Windows stable PT-10 crash-schedule property rerun with
  10,000/10,000 cases passing in 1,325.25 seconds (exit code `0`); retained
  the structured record and raw-log SHA-256 under
  `artifacts/m1-property-runs/`. This closes PT-10's Windows stable
  10,000-case evidence only; PT-14 and release-owner acceptance remain open,
  and M1 remains **not passed**.
- Added three bounded concurrent Windows PT-14 diagnostic records using
  independent target directories (256 requested cases across all nine
  migration failpoints per run); all exited zero with no observed host error.
  Raw logs and SHA-256 values are retained in
  `artifacts/m1-property-runs/windows-stable-pt14-diagnostic-2026-08-27.json`.
  These probes are supplemental diagnostics only; the normative PT-14 run,
  cross-platform fault matrix, and M1 release-owner acceptance remain open.
- Audited the accepted event-model documentation against the production
  metadata schema and recorded the missing event envelope/projection contract
  in `M1_EVIDENCE.md`. The current event store remains a per-stream append log;
  no projection schema, cursor, handler, or rebuild API is claimed, so PT-13
  and FI-10 remain unimplemented and M1 remains **not passed**.
- Added proposed [`ADR-0016`](../decisions/ADR/ADR-0016-projection-contract.md)
  to define the generation-bound projection contract needed before FI-10/PT-13
  can be implemented; the ADR is pending and does not change the M1 gate.
- Fixed a Rust 1.78-only `clippy::needless_borrows_for_generic_args` failure in
  `tests/artifact_consistency.rs`, then reran the Windows stable and MSRV
  `fmt/check/test/clippy` gates in fresh target directories. All commands and
  the artifact-consistency test exited zero; this does not change the M1 gate
  decision or provide missing Linux/native/old-reader evidence.
- Re-ran the complete Windows stable Rust 1.95.0 and Rust 1.78.0 MSRV
  `fmt/check/test/clippy` gates on 2026-08-27 in independent close-out target
  directories, recording exit code `0` for every command and a passing
  artifact-consistency test under each toolchain. This is refreshed local
  evidence only; M1 remains **not passed** and all external/implementation
  blockers remain visible.
- An earlier audit note incorrectly described the checkout as lacking `.git`
  metadata. The repository is on branch `dev` at
  `2da6cf1037acf054b33b23b07c27388be1807690` with `origin`; the working tree
  is dirty and no release tag/commit is accepted, so the evidence package does
  not claim a clean release artifact.
- Corrected broken relative links in ADR-0014 and ADR-0015 so the M1 storage,
  protocol, recovery, gate, and performance artifact references resolve from
  their `docs/decisions/ADR/` location.
- Completed the Windows stable PT-14 normative rerun with
  `PONG_PROPTEST_CASES=10000`: 10,008 executed cases across nine migration
  failpoints, `1 passed; 0 failed`, exit code `0`, and no observed host error or
  mixed-generation state. The structured record and raw-log SHA-256 are
  retained under
  `artifacts/m1-property-runs/windows-stable-pt14-10000-rerun-2026-08-27.json`
  and `.log`; this closes only the Windows stable property row, not the
  cross-platform matrix or M1 release-owner acceptance.
- The PT-14 raw log capture method is recorded as session-output reconstruction
  (not a fresh tee rerun); its SHA-256 is verified by the read-only artifact
  consistency test, and this capture caveat does not widen the gate decision.

## Unreleased - M1 native evidence run #4 audit (2026-08-28)

- Retained GitHub Actions run `33142438624` and both complete native evidence
  artifacts. All native format/check/clippy and focused M1 commands passed;
  both full test commands on Linux and macOS failed at the same
  `artifact_consistency` assertion.
- Identified the deterministic cause: the committed LF `build-metadata.json`
  blob differed from stale Windows CRLF digest/reference values. Rebound the
  release `SHA256SUMS` and `artifact-references.json` records to the committed
  LF bytes and verified a clean-index export plus local artifact-consistency
  tests. M1 remains `NOT PASSED`; one fresh native dispatch is required.

## Unreleased - M1 native evidence run #2 (2026-08-27)

- Retained GitHub Actions run `33080915116` at commit
  `8d28075d44f5458e866c94ee33b95b430f7959d6` and both downloaded native
  evidence ZIPs. Linux SHA-256 is
  `B3E457A9B936347E619C280B5DB5ACE06A9C7A7C206178193D85527BDF9E0B1E`;
  macOS SHA-256 is
  `01A1BC12E9DE8051F603EEDA3CC23A920419067F38503F38635856628083F16F`.
- Recorded Linux stable/MSRV full-test failures in `artifact_consistency` due
  retained evidence hash drift after checkout. The focused migration,
  recovery, compatibility, PT-13/FI-10, and cold-reopen commands passed.
- Recorded the macOS artifact boundary honestly: only the Rust 1.78 build logs
  and focused logs were produced; MSRV test failed while MSRV clippy exited
  zero, and platform metadata, manifest, stable logs, and stable exit records
  are absent.
- Added the run to the M1 release matrix, compatibility matrix, roadmap, and
  checksum/reference bundle. Workflow capture-boundary hardening was pushed in
  `2cf136e`; M1 remains `NOT PASSED` and a fresh successful native run is
  required.
