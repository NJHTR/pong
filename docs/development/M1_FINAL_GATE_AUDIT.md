# M1 Final Gate Audit

**Audit date:** 2026-08-28  
**Branch:** `dev`  
**Audited HEAD:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Overall disposition:** `BLOCKED` / `M1_RELEASE_GATE = NOT_PASSED`

This is an evidence audit, not a release approval. `PASS` below means that the
named evidence was found and its stated command/result was independently
checked. It does not override a missing platform, compatibility, fault,
performance, traceability, or release-owner requirement.

> **Later decision-draft addendum (2026-08-29):** The supplied Owner Decision
> Draft records `OWNER_DECISION_DRAFT = APPROVED_FOR_FINALIZATION` for a bounded
> scope. Linux FI-13 and the Windows/Linux normative property corpus were
> completed after this audit. Current status is tracked in
> `M1_FINAL_CANDIDATE_READINESS.md`; the historical findings below are not
> rewritten as if they were known on the original audit date. Formal acceptance,
> ADR decisions, compatibility exception, clean candidate freeze, and sign-off
> remain pending.

> **Final acceptance overlay (2026-08-29):** Release Owner `NJHTR` has now
> accepted the bounded scope, Old Reader exception, FI-03 deferment,
> informational performance baseline, ADR-0016 projection contract, and
> textual sign-off. The historical findings below remain audit inputs. The
> only current release blocker is a clean committed candidate freeze and
> post-freeze verification; see `M1_FINAL_GATE_REPORT.md`.

## Scope and Method

The audit read and cross-checked:

- `M1_EVIDENCE.md`, `M1_COMPATIBILITY_MATRIX.md`,
  `M1_CI_EXECUTION_REQUIRED.md`, and `M1_DURABLE_PRIMITIVES_GATE.md`;
- `ADR-0016-projection-contract.md` and `ADR-0015-m1-performance-capacity-budget.md`;
- `NEXT_TASK.md` and `CHANGELOG.md`;
- the release matrix, fault matrix, compatibility audit, traceability record,
  performance package, native-runner artifacts, and nested checksums;
- `src/lib.rs`, `src/metadata.rs`, `src/repository.rs`, the projection and
  fault tests, and `tests/artifact_consistency.rs`.

The current workspace was also checked with an isolated target directory:

```text
CARGO_TARGET_DIR=D:\pong\target\m1-final-gate-audit-20260828
cargo fmt --all -- --check                         exit 0
cargo check --locked                              exit 0
cargo test --locked                               exit 0
cargo clippy --locked --all-targets -- -D warnings exit 0
cargo test --locked --test artifact_consistency -- --nocapture exit 0
```

The workspace is dirty because the previously imported Run #5 evidence and
its documentation updates are uncommitted. No production code was changed by
this audit.

## Gate Audit

| Gate | Current status | Existing evidence | Missing evidence | Blocking | Next step |
|---|---|---|---|---:|---|
| Build | `PASS` | Local stable and MSRV 1.78 isolated builds; Run `33145714975` stable/MSRV build records on Linux and macOS | No additional build failure found | No | Preserve the exact commit/target/toolchain records in the final release snapshot |
| Test | `PASS` | Local full suite passed; Run `33145714975` recorded stable/MSRV full tests and focused suites as exit `0` on both native jobs | No additional test failure found | No | Keep the successful native artifacts bound to the release commit |
| Clippy | `PASS` | Local `-D warnings` pass; Run `33145714975` stable/MSRV clippy records exit `0` on both native jobs | No additional lint failure found | No | Re-run only if the release commit changes |
| Native Linux | `PASS` (scoped evidence) | Run `33145714975`, Ubuntu 24.04 x86_64, repository filesystem `ext4`, complete manifest, nested checksums, 13/13 commands exit `0` | Release-owner acceptance and non-platform M1 rows | Yes, at release level | Accept the row in the release-owner record; do not conflate it with the whole M1 gate |
| Native macOS | `PASS` (scoped evidence) | Run `33145714975`, macOS 14 arm64, complete manifest, nested checksums, 13/13 commands exit `0` | Filesystem is explicitly `unknown`; release-owner acceptance and non-platform M1 rows | Yes, at release level | Keep filesystem `unknown`; obtain owner disposition for the declared target |
| Old-reader compatibility | `BLOCKED` | `compatibility/old-reader-audit.json` and `.log` explicitly record no reader, source, execution, or exit code; current-source fixtures pass only the in-tree boundary | Separately released historical Pong v0.1 reader binary, version/tag, SHA-256, fixture, read probe, mutation/open probe, and retained output | Yes | Obtain a real old reader or record an explicit owner-approved compatibility exception; never use a mock/current reader |
| Event / Projection | `NOT_PROVEN` | ADR-0016 implementation exists; `event_projection.rs` and `pt13_fi10.rs` pass locally and in Run #5 focused commands; generation, cursor, ledger, digest, rebuild, and failpoint checks are present | ADR-0016 is still `Proposed`; independent accepted fixture/cross-platform projection evidence and release-owner acceptance are absent | Yes | Close ADR-0016 and retain the required versioned fixture, cold-reopen, and source-immutability evidence |
| Fault injection | `BLOCKED` | `artifacts/m1-fault-matrix.json` records executable synthetic/process/WAL/security coverage; Run #5 runs the focused PT-13/FI-10 schedule | FI-03 external tool-effect schedule; FI-07/FI-13/FI-14 complete declared-filesystem/resource matrix; native power-loss/filesystem disposition and owner acceptance | Yes | Complete or explicitly bound every required FI row; retain raw OS status, exit code, cold reopen, and disposition |
| Property testing | `NOT_PROVEN` | PT-01..PT-12 and PT-14 have retained Windows stable corpus evidence; native jobs execute the standard property tests; PT-14 normative record reports 10,008 cases and zero failures | Normative corpus and evidence on every accepted platform/filesystem row; PT-13 projection acceptance; complete cross-platform property manifest | Yes | Run the declared corpus on each accepted row or record an owner-approved bounded exception |
| Performance | `NOT_PROVEN` | `m1-perf-0.3` measurements retain p50/p95/p99/max, CAS throughput, migration, and snapshot scales; repeated Windows/Linux-overlay raw runs exist | ADR-0015 remains `Proposed`; no accepted threshold decision, no three-run evidence for native Linux ext4/macOS, and no owner exception | Yes | Release owner must accept ADR-0015 against the accepted platform matrix, or record a compensating bound |
| Artifact consistency | `PASS` (bundle scope) | Local `artifact_consistency` test passed; Run #5 manifests, command dispositions, nested `SHA256SUMS`, commit/run IDs, and ZIP digests are retained and match the imported artifacts | External GitHub artifact remains an external source; bundle test cannot itself create owner approval or fix stale traceability metadata | No as a standalone evidence check | Re-run after any bundle mutation and bind the final bundle to the release commit |
| Traceability | `BLOCKED` | Native artifacts and regenerated traceability identify commit `6cb62fb`; bundle checksums are consistent | Branch has no release tag, working tree contains uncommitted evidence/documentation changes, and owner approval is pending | Yes | Bind the final clean evidence snapshot to the release candidate commit/tag through the normal owner-controlled process |
| Release owner sign-off | `BLOCKED` | Matrix and ADR acceptance tables intentionally say `UNASSIGNED` / `PENDING` | Named owner, decision date, accepted platform/filesystem scope, accepted old-reader/fault/performance dispositions, and signed record | Yes | A human release owner must review the retained bundle and record the decision; the audit agent cannot self-approve |

## Fault Coverage Matrix

This table distinguishes implementation, execution, and release evidence. A
synthetic test is not silently promoted to native or power-loss evidence.

| Fault ID | Required | Implemented | Executed | Evidence | Result |
|---|---:|---:|---:|---|---|
| FI-01 | Yes | Yes | Yes | `m1-fault-matrix.json`; synthetic/file-backed cold reopen | `PASS` scoped; owner/native disposition pending |
| FI-02 | Yes | Yes | Yes | process-kill and cold-reopen tests | `PASS` scoped; external schedule pending |
| FI-03 | Yes | Partial | Partial | property crash schedule only; no external tool hook | `BLOCKED` |
| FI-04 | Yes | Yes | Yes | WAL-tail and outcome-boundary tests | `PASS` scoped; power-loss disposition pending |
| FI-05 | Yes | Yes | Yes | process-kill/outbox recovery tests | `PASS` scoped; full platform schedule pending |
| FI-06 | Yes | Yes | Yes | SQLite commit failpoints and property corpus | `PASS` scoped; native power-loss pending |
| FI-07 | Yes | Yes | Yes | synthetic short-write plus Linux `tmpfs` ENOSPC | `BLOCKED` for complete declared platform matrix |
| FI-08 | Yes | Yes | Yes | CAS directory-sync failpoint and cold reopen | `PASS` scoped; filesystem-specific disposition pending |
| FI-09 | Yes | Yes | Yes | ref CAS and generated writer-order property test | `PASS` scoped |
| FI-10 | Yes | Yes | Yes | projection A-J schedule, rebuild, migration interruption | `NOT_PROVEN` beyond current-host/accepted-owner scope |
| FI-11 | Yes | Yes | Yes | redaction and full `.pong` byte-scan tests | `PASS` scoped; external export integrations pending |
| FI-12 | Yes | Yes | Yes | migration process-kill and foreign-generation identity tests | `PASS` scoped; owner/native disposition pending |
| FI-13 | Yes | Partial | Yes | Windows ACL record; no complete filesystem schedule | `BLOCKED` |
| FI-14 | Yes | Partial | Yes | Linux disposable `tmpfs` CAS/metadata/journal records | `BLOCKED` for Windows/other declared rows |

## Property Coverage Matrix

| Property ID | Current evidence | Result |
|---|---|---|
| PT-01, PT-02, PT-03, PT-04, PT-05, PT-06, PT-07, PT-08, PT-11, PT-12 | Windows stable retained 10,000-case records plus normal local/native execution | `PASS` scoped; cross-platform normative acceptance not proven |
| PT-09 | Windows stable retained 10,000-case recovery rerun | `PASS` scoped; cross-platform normative acceptance not proven |
| PT-10 | Windows stable retained 10,000-case crash schedule rerun | `PASS` scoped; cross-platform normative acceptance not proven |
| PT-13 | Projection/migration semantic and failpoint tests; current-host evidence and Run #5 focused execution | `NOT_PROVEN` for final release acceptance while ADR-0016/owner disposition is pending |
| PT-14 | Windows stable normative record: 10,008 cases, 9 migration failpoints, zero failures; focused native execution also passed | `PASS` scoped; normative runs on all accepted rows are absent |

## Performance Audit

The retained `m1-perf-0.3` aggregate reports the following representative
sample counts and extrema. Values are measurement evidence, not an accepted
budget:

| Operation | Samples | p95 | p99 | Max | Proposed ADR-0015 limit |
|---|---:|---:|---:|---:|---:|
| Repository open | 64 | 32.488 ms | 38.578 ms | 41.425 ms | 100 / 250 ms |
| Metadata intent + outcome | 64 | 0.328 ms | 0.401 ms | 4.463 ms | 10 / 25 ms |
| Event append | 64 | 0.257 ms | 0.342 ms | 0.358 ms | 10 / 25 ms |
| Recovery mark intents unknown | 8 | 0.698 ms | 0.698 ms | 0.746 ms | 100 / 250 ms |
| Repository migration | 8 | 89.698 ms | 89.698 ms | 93.532 ms | 1,000 / 3,000 ms |
| CAS publication | 32 objects | n/a | n/a | 14.669 MiB/s | >= 1 MiB/s |

The performance package explicitly states that native Linux ext4, native
macOS, and Windows native disk-full rows lack the required three-run package.
Because ADR-0015 is not accepted, the correct gate state is `NOT_PROVEN`, not
`PASS`, even where the observed samples are below the proposed limits.

## Compatibility and Traceability Findings

1. `old-reader-audit.json` contains no reader identity or execution result,
   and its log confirms that no separately released v0.1 binary/tag/release
   exists. Current-source compatibility fixtures are useful supplementary
   evidence but cannot establish backward compatibility with an old reader.
2. Run `33145714975` is the authoritative native evidence snapshot for this
   audit: commit `6cb62fb`, Linux filesystem `ext4`, macOS filesystem
   `unknown`, and all 13 command records per platform equal to zero.
3. `release-traceability.json` has been regenerated for the audited HEAD
   `6cb62fb`. It correctly records a dirty working tree, no release tag, and
   pending owner approval. Traceability therefore remains `BLOCKED` for
   release purposes even though the retained native artifact checksums are
   internally consistent.
4. The Node.js 20 deprecation annotations in the workflow are warnings. They
   are not interpreted as M1 failures and do not change the validation scope.

## Ranked Blockers

1. **Old-reader compatibility (`BLOCKED`, highest):** no real old reader,
   fixture invocation, or read/write probe exists.
2. **Release-owner governance (`BLOCKED`):** no named owner, decision date,
   accepted scope, or sign-off; this independently prevents `PASS`.
3. **Fault matrix completeness (`BLOCKED`):** FI-03 remains external and
   FI-07/FI-13/FI-14 remain partial host/filesystem matrices; FI-10 is only
   current-host evidence pending acceptance.
4. **Performance budget (`NOT_PROVEN`):** ADR-0015 is proposed and lacks
   accepted thresholds across every accepted platform/filesystem row.
5. **Property scope (`NOT_PROVEN`):** retained normative high-load corpus is
   Windows-focused; cross-platform accepted-row evidence is incomplete.
6. **Projection contract acceptance (`NOT_PROVEN`):** ADR-0016 is proposed,
   and current implementation evidence is not the same as final contract
   acceptance.
7. **Release traceability (`BLOCKED`):** no release tag and the retained
   traceability JSON points at an earlier snapshot.

## Audit Conclusion

```text
BUILD = PASS
TEST = PASS
CLIPPY = PASS
NATIVE_LINUX = PASS (scoped executable evidence)
NATIVE_MACOS = PASS (scoped executable evidence; filesystem unknown)
ARTIFACT_CONSISTENCY = PASS (retained bundle scope)
OLD_READER = BLOCKED
FAULT_MATRIX = BLOCKED
PROPERTY_TESTS = NOT_PROVEN
EVENT_PROJECTION = NOT_PROVEN
PERFORMANCE = NOT_PROVEN
TRACEABILITY = BLOCKED
RELEASE_OWNER_SIGNOFF = BLOCKED

M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```

No M2 work should start from this audit state. The next action is to close or
explicitly disposition the ranked blockers above; it is not to re-run the
already successful native matrix or to relabel missing evidence.
