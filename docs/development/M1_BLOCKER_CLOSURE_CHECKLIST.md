# M1 Blocker Closure Checklist

**Audit date:** 2026-08-28  
**Audited branch:** `dev`  
**Audited HEAD:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Current decision:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`

This is a readiness audit, not a release approval. It does not modify
production code, accept an ADR, create a tag, or sign on behalf of the Release
Owner.

> **Later decision-draft addendum (2026-08-29):** The bounded scope is now
> `APPROVED_FOR_FINALIZATION` in `M1_OWNER_DECISION_RECORD.md`. Linux FI-13 and
> the Windows/Linux normative property corpus were completed after the original
> audit sections below. Current technical/governance separation is recorded in
> `M1_FINAL_CANDIDATE_READINESS.md`. This addendum does not execute the Old
> Reader exception, accept an ADR, sign the release, or rewrite the historical
> audit.

> **Final acceptance addendum (2026-08-29):** Release Owner `NJHTR` formally
> accepted the bounded M1 scope and governance decisions by textual owner
> declaration. Old Reader is an accepted first-release exception, FI-03 is
> deferred to M2+, ADR-0015 is accepted as an informational baseline, and
> ADR-0016 is accepted. These decisions supersede the pending-owner
> dispositions in the historical audit sections below, but they do not freeze
> a candidate or create a release tag. The current release state is
> `TECHNICAL_GATE = PASS (scope-bounded)`, `GOVERNANCE_GATE = ACCEPTED`,
> `CANDIDATE_FREEZE = PENDING`, `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`.

The status vocabulary is intentional:

- `PASS` means that the named executable evidence was found and checked for
  its stated scope. It does not mean that the Release Owner accepted the
  scope.
- `NOT_PROVEN` means that the implementation or partial evidence exists, but
  the required proof set is incomplete. It is not `FAIL`.
- `BLOCKED` means that a known required artifact, external execution, or
  governance decision is absent. Synthetic or current-source evidence is not
  silently promoted to external evidence.

The audit uses [`M1_DURABLE_PRIMITIVES_GATE.md`](M1_DURABLE_PRIMITIVES_GATE.md),
[`M1_FINAL_GATE_AUDIT.md`](M1_FINAL_GATE_AUDIT.md),
[`M1_FINAL_GATE_CLOSURE_PLAN.md`](M1_FINAL_GATE_CLOSURE_PLAN.md),
[`M1_EVIDENCE.md`](M1_EVIDENCE.md),
[`M1_COMPATIBILITY_MATRIX.md`](M1_COMPATIBILITY_MATRIX.md),
[`M1_PERFORMANCE_ACCEPTANCE.md`](M1_PERFORMANCE_ACCEPTANCE.md),
[`M1_RELEASE_OWNER_SIGNOFF.md`](M1_RELEASE_OWNER_SIGNOFF.md), ADR-0015, and
ADR-0016.

## Execution Classification

This closure pass classified work before running it. The classification does
not promote evidence between environments:

| Class | What can be done | Result in this pass |
| --- | --- | --- |
| A. Direct repository tests | Projection/FI-10 tests, artifact consistency, local property corpus, and measurement probe | Executed on the current Windows host; all selected commands exited `0`. These are current-host evidence. The default local property run uses the repository's local case counts (storage-heavy properties are `128`, not the normative `10,000`), so it does not close the normative cross-platform corpus requirement. The performance output is supplementary because it was written outside the retained release bundle. |
| B. Linux native environment | Native Linux filesystem/resource schedules, including FI-14 | Not executable in this workspace: Docker Desktop's Linux daemon was unavailable (`docker version` could not connect). Existing Linux records are retained evidence for their declared Docker `tmpfs`/overlay/ext4 environments, not a new native-Linux result. |
| C. GitHub Actions native runner | GitHub-hosted Linux and macOS quality/focused runs | No new workflow was dispatched in this pass. Existing Run `33145714975` remains the authoritative GitHub-hosted evidence; macOS filesystem remains `unknown` and is not called APFS or a physical Mac. |
| D. Historical artifact / old-reader | Independent v0.1 executable, fixture, and mutation/read probes | Repository history, tags, branches, docs, and retained compatibility records were checked; no independent reader was found. |
| E. Release Owner acceptance | ADR decisions, compatibility/fault/property scope exceptions, traceability release-candidate decision, and sign-off | Not executable by Codex. These remain human decisions and are not inferred from green tests. |

### Blocker-to-class mapping

| Blocker | A: repository test | B: Linux native | C: GitHub-hosted runner | D: historical artifact | E: owner decision |
| --- | --- | --- | --- | --- | --- |
| Old Reader | Boundary fixtures only; cannot create an old reader | No | No | **Required** | Exception possible only as an explicit scope exclusion |
| Fault Matrix | Synthetic FI rows and current-host checks | FI-07/FI-14 resource rows where a real Linux environment is available | Additional accepted-row execution | No | Scope and residual-risk disposition required |
| Property Testing | Local/proptest execution | Normative corpus on accepted Linux rows | Normative corpus on accepted GitHub-hosted rows | No | Scope exceptions and acceptance required |
| ADR-0015 Performance | Measurement probe | Measurements for accepted Linux rows | Measurements for accepted GitHub-hosted rows | No | **Required** to accept thresholds or a compensating bound |
| ADR-0016 Projection | Focused contract/FI-10 tests | Only if the accepted scope requires it | Existing Run #5 focused evidence | No | **Required**; not waiverable while projections remain in M1 |
| Traceability / Release Candidate | Checksum/consistency verification | No | Workflow provenance | No | Repository owner must freeze the clean candidate |
| Release Owner Sign-off | No | No | No | No | **Required** human action |

## This Pass: Executed Evidence

The following commands were run with independent target directories on the
current Windows host:

```text
cargo test --locked --test event_projection --test pt13_fi10 -- --nocapture       exit 0
cargo test --locked --test artifact_consistency -- --nocapture                    exit 0
cargo test --locked --test property_corpus -- --nocapture                        exit 0
cargo test --locked --test performance_measurements -- --ignored --nocapture     exit 0
cargo test --locked --test host_resource_faults \
  fi_13_real_windows_acl_read_revocation_fails_closed_and_recovers \
  -- --ignored --nocapture                                                       exit 0
```

The FI-13 terminal output recorded `platform=windows`, `filesystem=NTFS`,
kernel error `5`, Pong code `PERMISSION_DENIED`, and successful cold reopen
after ACL restoration. This fresh output was not copied into the retained
release bundle; the existing retained FI-13 artifact remains the release
evidence record. The performance probe wrote an `m1-perf-0.3` record with Windows
stable Rust `1.95.0`, four operation measurement groups, eight migration
samples, three snapshot scales, and approximately `9.796 MiB/s` CAS throughput;
it is measurement evidence only and does not accept ADR-0015. No selected test
exposed a production defect. No Linux resource test was run after Docker
reported an unavailable daemon, and no macOS test was fabricated.

## Closure Table

| Blocker | Current | Required Evidence | Existing Evidence | Missing | Action | Owner | Can Waive |
| ------- | ------- | ----------------- | ----------------- | ------- | ------ | ----- | --------- |
| Old Reader | `BLOCKED` | An independently released historical Pong v0.1 reader; exact version/source identity; immutable binary SHA-256; fixture created by that reader; read/open probe; mutation-before/after-selector probe; raw stdout/stderr, exit codes, and cold-reopen result. | `artifacts/m1-release-evidence/compatibility/old-reader-audit.json` and `.log` explicitly record no reader, source, binary, fixture, execution, or exit code. Current-source v0.1 fixtures and selector boundary tests are supplementary only. | No separately released v0.1 executable, tag, release, binary hash, or historical fixture exists in the checked history or remote metadata. | Obtain and execute a real old reader, or obtain a signed compatibility-scope exception. Never compile the current reader and call it old. | Compatibility owner supplies the artifact and probe, or the Release Owner records that v0.1 backward compatibility is outside the supported M1 scope. | Compatibility owner / Release Owner | `CONDITIONAL`: only a signed scope exception that explicitly excludes the unsupported legacy reader. The row remains unverified and legacy repositories may be unreadable. |
| Fault Matrix | `BLOCKED` | FI-01 through FI-14 must have deterministic repeated execution, named fault point, platform/filesystem identity, raw OS/provider error, process exit status, cold restart inspection, and old-or-new/unknown/quarantine result. FI-03 requires a real external tool effect followed by termination. FI-07, FI-13, and FI-14 require complete evidence for every accepted platform/filesystem row. | `artifacts/m1-fault-matrix.json`; `artifacts/m1-release-evidence/fault/m1-fault-scenarios.json`; synthetic failpoints; Windows NTFS ACL evidence; Linux 64 MiB tmpfs CAS/metadata/journal evidence; current-host PT-13/FI-10 schedule. | FI-03 has no external provider/tool execution hook. FI-07/FI-13/FI-14 are partial platform/filesystem matrices; Windows native disk-full/quota and other accepted rows are absent. Native power-loss is not claimed. | Complete the missing host/provider schedules in disposable environments and retain raw records, or explicitly remove the affected environment/operation from the supported release scope. | Reliability/runtime owner plus storage/platform owner; Release Owner for any scope exception. | `CONDITIONAL`: FI-07/FI-13/FI-14 may be excluded only by an explicit platform/filesystem scope exception. FI-03 cannot be waived for a release that claims safe external side-effect execution; an exclusion must state that such execution is unsupported. Residual risk is unverified corruption, resource exhaustion, or unknown side effects. |
| Property Testing | `NOT_PROVEN` | PT-01 through PT-14 require deterministic generator/corpus metadata, seed, requested and executed case count, invariant result, shrink/failure trace when relevant, platform/filesystem identity, and retained raw output. The normative default is at least 10,000 cases per property in CI; PT-13/PT-14 also require cold reopen and source/generation checks. | Retained Windows stable PT-01..PT-12 and PT-14 records, including PT-09/PT-10 reruns and PT-14 normative `10,008` cases with zero failures. Native Run `33145714975` executes the standard suites on Linux and hosted macOS. | A normative corpus record is not retained for every accepted platform/filesystem row. PT-13 evidence is executable, but its release acceptance is pending ADR-0016 and owner disposition. | Run the declared corpus on each accepted row and retain machine-readable records, or record a bounded owner exception that narrows the accepted platform/filesystem scope. | Reliability/property-test owner; Release Owner for scope exception. | `CONDITIONAL`: a bounded exception may exclude unproven rows from the supported scope. It does not make those properties `PASS`; the risk is platform-specific invariant or migration divergence. |
| ADR-0015 Performance Acceptance | `OWNER_ACTION_REQUIRED` | Measurement package for each accepted platform/filesystem row, with repeat count (three runs where required), operation scale, raw samples, p50/p95/p99/max, CAS throughput, migration/recovery results, and an explicit accepted threshold or signed exception with a compensating bound. | Proposed ADR-0015; `docs/development/M1_PERFORMANCE_ACCEPTANCE.md`; `m1-perf-0.3` raw package; three Windows stable, three Windows Rust 1.78, and three pinned Linux Docker-overlay runs; representative operation aggregates; this pass also ran one current-Windows measurement probe. | ADR-0015 is still `Proposed`. No complete three-run package exists for native Linux ext4 or hosted macOS, and no Windows native disk-full/quota performance row exists. Memory/CPU measurements are absent and have no owner-approved scope decision. | Complete measurements for the accepted scope without changing thresholds to fit the results, then have the Release Owner accept ADR-0015 or record a compensating bound. | Performance owner / Release Owner. | `YES, CONDITIONAL`: the owner may accept a bounded capacity scope and explicitly waive missing rows or memory/CPU claims. Risk: no performance or capacity guarantee outside the bound; regressions may remain undetected. This is not automatic ADR acceptance. |
| ADR-0016 Projection Contract Acceptance | `OWNER_ACTION_REQUIRED` | Accepted ADR-0016 decision plus versioned/golden projection fixtures; unknown and unsupported schema vectors; duplicate/conflicting event tests; cursor ordering; generation identity; forward/backward migration with opaque-field preservation; FI-10 crash/checkpoint/verification/publication schedule; cold reopen; source-event immutability; and release-owner acceptance. | `docs/decisions/ADR/ADR-0016-projection-contract.md` contains the contract and remains `Proposed`. `tests/event_projection.rs` and `tests/pt13_fi10.rs` pass locally in this pass and in the Run #5 focused evidence. Current evidence covers envelope, cursor, ledger, digest, rebuild, idempotency, unknown-event handling, generation binding, and A-J failpoints. | No accepted ADR decision, signed owner disposition, or release-level acceptance record exists. The current executable evidence is not by itself contract acceptance. | Core/release owner reviews the retained fixtures and tests, accepts or rejects ADR-0016, and records the decision without weakening the stated invariants. | Core owner / Release Owner. | `NO` for a release that includes the projection contract. It may only be deferred by a formal scope/roadmap decision that removes projections from M1; that is a scope change, not a waiver. Risk of waiver is ambiguous replay, migration, cursor, and projection semantics. |
| Traceability / Release Candidate | `BLOCKED` | A clean release-candidate source commit must map to the exact workflow run, platform artifacts, fault/property/performance/compatibility records, nested checksums, dependency/toolchain identities, and final decision. The candidate version/tag and working-tree state must be explicit. | `artifacts/m1-release-evidence/traceability/release-traceability.json` and `.log` identify HEAD `6cb62fb` and native Run `33145714975`. `artifact_consistency` passes its current assertions; top-level `SHA256SUMS` verifies its entries. | Working tree is `DIRTY`; no release tag or candidate version exists; owner approval is pending. A packaging audit also found 445 selected `artifact-references.json` entries versus 451 `SHA256SUMS` entries, six retained Linux VM `*-exact` files outside that selected index, and a stale self-entry. The index is not specified as a one-to-one checksum mirror, and the self-hash is circular; reconcile or label this hygiene caveat before candidate freeze without adding a new Gate condition. Traceability is evidence for the current snapshot, not a release candidate. | Freeze the final evidence snapshot, regenerate or explicitly label the selected reference index (excluding/handling its self-entry), verify top-level checksums, rerun consistency against that exact commit, and let the repository owner create the candidate tag through the normal controlled process. | Release/repository owner. | `NO`: provenance and clean-candidate requirements cannot be waived while calling the result a release candidate. Without them there is no reproducible release claim. |
| Release Owner Sign-off | `OWNER_ACTION_REQUIRED` | Named human owner, decision date, exact commit and scope, accepted evidence list, accepted exceptions and risks, decision (`ACCEPTED` or `REJECTED`), and signature in `M1_RELEASE_OWNER_SIGNOFF.md`. | `docs/development/M1_RELEASE_OWNER_SIGNOFF.md` intentionally remains `UNASSIGNED` / `PENDING`. Matrix and ADR acceptance tables also remain pending. | No named owner, date, accepted scope, exception record, decision, or signature exists. The implementation agent cannot self-approve. | Release Owner reviews the retained bundle and completes the human approval record after all technical dispositions are known. | Release Owner. | `NO`: sign-off is the governance control itself. A missing signature cannot be waived by the same unsigned record. |

## 1. Old Reader

**Current status:** `BLOCKED`.

**Why it cannot PASS:** The repository history and remote metadata contain no
separately released Pong v0.1 reader. The current Rust reader and current-source
compatibility fixtures are not independent historical readers. Therefore there
is no executable evidence that an older supported executable can read the M1
repository or fail closed when the selector or generation is mutated.

**Exact Gate evidence required:** A binary independent of the current reader
source, its version/source identity and SHA-256, a fixture produced by that
binary, a read/open probe against the current repository, a mutation probe, raw
outputs and exit codes, and a cold-reopen result. The probe must demonstrate
that an old reader cannot silently mutate or misinterpret the new selector or
generation.

**Evidence already in the repository:** The absence record
`old-reader-audit.json` / `.log`; current-source legacy SQLite fixtures;
selector marker tests; and current-reader migration/integrity tests. These are
useful boundary evidence but do not prove historical binary compatibility.

**Missing:** The old binary, immutable hash, fixture, and all probe outputs.

**Can a real test solve it?** Yes, but only after a real historical reader and
fixture are supplied. No test built from the current source can close this
gap.

**External dependency:** Yes. It requires a historical release artifact or an
independent archival build and its provenance.

**Waiver:** A Release Owner may sign a bounded compatibility exception that
explicitly removes v0.1 backward-read support from the M1 release scope. That
exception must not relabel the row `PASS`. Risk: users with legacy repositories
may be unable to open or migrate them, and the compatibility claim becomes
"current-reader migration only".

**Minimum plan if not waived:** Obtain the real reader, record its hash and
fixture, run read/mutation/cold-reopen probes, update the compatibility matrix,
and rerun artifact consistency on the exact candidate.

## 2. Fault Matrix

**Current status:** `BLOCKED`.

**Why it cannot PASS:** Several rows are covered by synthetic or partial
host evidence, but the required matrix distinguishes those from real external
and platform evidence. FI-03 has no external tool-effect schedule. FI-07,
FI-13, and FI-14 do not cover every accepted filesystem/platform row. Native
power-loss is not claimed merely because a process-kill or failpoint test is
green.

**Exact Gate evidence required:** Every FI-01..FI-14 case needs a deterministic
fault point, repeated execution, platform/filesystem identity, raw OS or
provider result, process status, cold restart inspection, and a stable outcome
such as `unknown`, `RESOURCE_EXHAUSTED`, quarantine, old-only, or verified-new.
FI-03 must show a real tool effect followed by termination before outcome
append. The matrix must not substitute synthetic evidence for native evidence.

**Evidence already in the repository:**
`artifacts/m1-fault-matrix.json`,
`artifacts/m1-release-evidence/fault/m1-fault-scenarios.json`, Windows NTFS
ACL evidence, Linux 64 MiB tmpfs CAS/metadata/journal evidence, process-kill,
WAL, CAS, migration, redaction, and current-host projection tests.

**Missing:** The FI-03 external schedule and complete FI-07/FI-13/FI-14 rows
for every accepted environment, including the required raw resource/error and
cold-reopen records.

**Can a real test solve it?** Partly. Host-resource and permission rows can be
closed with controlled disposable-host tests. FI-03 also needs an external
provider/tool harness; a local synthetic failpoint cannot establish provider
reconciliation behavior.

**External dependency:** Yes for external tool effects, native filesystem
coverage, and any power-loss or quota environment not available on the current
host.

**Waiver:** Only conditionally. The owner may exclude a specific unsupported
platform/filesystem or external-side-effect operation from the release scope.
The owner may not claim safe external execution while waiving FI-03, and may
not call a partial matrix complete. Risk: unverified corruption, resource
classification, quarantine, or unknown side-effect behavior in the excluded
scope.

**Minimum plan if not waived:** Run the documented disposable-host procedures
from `tests/HOST_RESOURCE_FAULTS.md`, add the external FI-03 schedule, retain
raw logs/status/error codes and cold-reopen inspection for every accepted row,
then update the fault matrix without promoting synthetic rows.

## 3. Property Testing

**Current status:** `NOT_PROVEN`.

**Why it cannot PASS:** The retained high-count corpus is primarily Windows
stable evidence. Native Linux and hosted macOS execute the normal suites, but
there is no retained normative corpus record for every accepted
platform/filesystem row. PT-13 also remains coupled to the pending ADR-0016
acceptance.

**Exact Gate evidence required:** For each PT-01..PT-14 row and each accepted
platform/filesystem, retain generator and invariant identity, deterministic
seed, requested/executed cases (default minimum 10,000 per property in CI),
shrink trace on failure, toolchain/filesystem identity, exit status, and raw
output. PT-13 and PT-14 require cold reopen plus source-event or
generation-identity checks.

**Evidence already in the repository:** Windows PT-01..PT-12 and PT-14
records, PT-09/PT-10 reruns, PT-14 normative `10,008` cases with zero failures,
diagnostic concurrent PT-14 probes, and native Run `33145714975` focused
execution.

**Missing:** Normative per-row records for all accepted native/platform
combinations and the final PT-13 contract acceptance.

**Can a real test solve it?** Yes. The gap is primarily evidence execution and
retention, not a request to lower the corpus count or change the invariants.

**External dependency:** Yes for any accepted native platform/filesystem not
available on the current host. A GitHub hosted macOS runner is valid hosted
runner evidence, but its filesystem remains `unknown` in the retained record;
it is not a physical Mac claim.

**Waiver:** A bounded exception can exclude unproven rows from the supported
scope. It does not turn them into `PASS`. Risk: platform-specific ordering,
recovery, or migration bugs can remain undiscovered.

**Minimum plan if not waived:** Run the declared corpus on each accepted row,
retain machine-readable manifests and raw logs, verify the case counts and
seeds, and bind the records to the candidate commit.

## 4. ADR-0015 Performance Acceptance

**Current status:** `OWNER_ACTION_REQUIRED`; ADR-0015 is `Proposed`.

**Why it cannot PASS:** Measurements exist and are below the proposed limits
for the recorded samples, but the limits have not been accepted and the
required platform/run package is incomplete. Native Linux ext4 and hosted
macOS lack the required three-run package, Windows native disk-full/quota is
not measured, and memory/CPU have no accepted disposition.

**Exact Gate evidence required:** Repeatable measurements by operation and
scale for every accepted platform/filesystem, raw samples and p50/p95/p99/max,
CAS throughput, migration and recovery timings, three independent runs where
specified, and a recorded ADR decision. Any omitted metric needs an explicit
scope decision; any exception needs a compensating operational bound.

**Evidence already in the repository:** `m1-perf-0.3`,
`docs/development/M1_PERFORMANCE_ACCEPTANCE.md`, three Windows stable runs,
three Windows Rust 1.78 runs, three pinned Linux Docker-overlay runs, and
representative aggregate timings.

**Missing:** Accepted ADR decision, complete native-row repetitions, and an
owner decision on memory/CPU scope.

**Can a real test solve it?** Tests and benchmark runs can supply the missing
measurements. They cannot themselves accept the ADR or choose the product's
capacity boundary.

**External dependency:** Yes for native Linux/macOS rows and any other
accepted environment unavailable locally. The owner decision is also an
external human dependency.

**Waiver:** Yes, conditionally. The owner may accept a bounded capacity scope
and record a compensating limit, such as supported repository size or
operation rate, while explicitly excluding unmeasured rows. Risk: no latency,
throughput, memory, or capacity guarantee outside that bound. Existing
measurements must not be relabeled as accepted thresholds.

**Minimum plan if not waived:** Execute the prepared measurement probe three
times per accepted row, preserve raw outputs and toolchain/filesystem identity,
complete the acceptance record, and have the owner decide ADR-0015.

## 5. ADR-0016 Projection Contract Acceptance

**Current status:** `OWNER_ACTION_REQUIRED`. Executable evidence is `PASS` for
the recorded current-host and Run #5 focused scope; release acceptance is
`PENDING` because ADR-0016 is still `Proposed`.

**Why it cannot PASS at the Gate:** A green implementation test is not an
architectural acceptance decision. The release still lacks the owner decision
that the generation-bound projection contract, fixture set, migration
semantics, and failure policy are the accepted M1 contract.

**Exact Gate evidence required:** A decision on ADR-0016 plus versioned golden
fixtures for envelopes and projections; unknown/unsupported versions;
duplicate and conflicting event behavior; deterministic cursor ordering;
generation and migration identity; forward/backward opaque-field preservation;
FI-10 handler/checkpoint/verification/publication crashes; cold reopen; and
source-event immutability.

**Evidence already in the repository:** ADR-0016's proposed contract;
`tests/event_projection.rs`; `tests/pt13_fi10.rs`; event envelope, cursor,
ledger, digest, rebuild, idempotency, unknown-event, generation-binding, A-J
failpoint, repeated-crash, and migration-interruption evidence. Run #5
focused projection commands also returned exit code `0`.

**Missing:** Formal acceptance or rejection of ADR-0016 and a release-level
record tying the evidence to that decision. This is not a request to redesign
the already implemented projection code.

**Can a real test solve it?** Tests can fill an evidence gap if a required
fixture or case is genuinely missing. Tests cannot accept an ADR; that is a
human architectural decision.

**External dependency:** The acceptance decision is human. Additional
cross-platform evidence, if required by the accepted scope, depends on the
corresponding runners.

**Waiver:** No for a release that includes projections in M1. The only valid
alternative is a formal scope/roadmap decision that defers projections from
M1, which changes the scope and is not a waiver of the contract.

**Minimum plan if not waived:** Core/release owner reviews ADR-0016 against
the retained tests and fixtures, records `Accepted` or `Rejected` with date and
scope, and keeps the Gate blocked if the contract is rejected or incomplete.

### ADR decision lines

```text
ADR-0015:
TECHNICAL EVIDENCE = m1-perf-0.3 retained Windows/Linux-overlay measurements plus one fresh current-Windows measurement; no accepted threshold decision.
OWNER DECISION REQUIRED = Accept the proposed budget for an explicitly named platform/filesystem and capacity scope, or record a compensating bound and exceptions.
WAIVER POSSIBLE = YES, conditionally; only as a signed bounded performance/capacity exception.

ADR-0016:
TECHNICAL EVIDENCE = event_projection and pt13_fi10 suites pass on the current Windows host and in GitHub-hosted Run 33145714975 focused jobs; envelope, cursor, ledger, rebuild, generation binding, and A-J failpoint behavior are retained.
OWNER DECISION REQUIRED = Accept or reject the Proposed generation-bound projection contract against the retained fixtures and invariants.
WAIVER POSSIBLE = NO while projections remain in M1 scope; deferral would be a formal scope change.
```

## 6. Traceability / Release Candidate

**Current status:** `BLOCKED` for release, although the current bundle's
internal consistency check is `PASS`.

**Why it cannot PASS:** The traceability record identifies HEAD and Run #5,
but the working tree is dirty, there is no release-candidate version or tag,
and owner approval is pending. The top-level `SHA256SUMS` verifies its listed
entries. A packaging audit found that the selected `artifact-references.json`
index has 445 records versus 451 checksum entries, six retained Linux VM
`*-exact` files outside that selected index, and a stale self-entry. Because
the index is not specified as a one-to-one checksum mirror and a self-hash is
circular, this is a hygiene caveat to reconcile or label before freezing the
candidate, not an additional Gate condition. A passing artifact check and the
current dirty snapshot are still not a clean, reproducible release candidate.

**Exact Gate evidence required:** One clean source commit must map to the
workflow run, platform artifacts, fault/property/performance/compatibility
records, nested checksums, dependency/toolchain identities, and final owner
decision. The candidate version/tag and clean working-tree state must be
recorded.

**Evidence already in the repository:**
`artifacts/m1-release-evidence/traceability/release-traceability.json` and
`.log`; Run `33145714975`; release manifests, references, `SHA256SUMS`; and a
passing `artifact_consistency` test. The selected-index packaging audit is
retained as a hygiene caveat, not a new M1 Gate row.

The traceability record and Run #5 active references point to
`6cb62fb455e92ab731a4bb5233856d10c1f1ce93`. Earlier failed workflow runs are
retained and explicitly labeled historical failure evidence; they are not the
active candidate. No current active reference was found pointing at a stale
commit.

**Missing:** A clean final commit, candidate version/tag, and owner approval.
The selected `artifact-references.json` index also needs regeneration or an
explicit selected/historical label before candidate freeze.

**Can a real test solve it?** No. Tests can verify checksums and references,
but only the repository/release process can create the clean candidate and its
provenance.

**External dependency:** Yes. It requires repository-owner control of the
commit/tag and the final evidence snapshot.

**Waiver:** No while making a release-candidate claim. A human may choose not
to publish a tag, but then the result remains an unreleased audit snapshot.
Risk of waiving provenance would be inability to reproduce exactly which
source and evidence were reviewed.

**Minimum plan if not waived:** Commit the final evidence/documentation state,
regenerate or label the selected `artifact-references.json` index and its
self-entry handling, verify top-level `SHA256SUMS`, verify `git status` is
clean, rerun `artifact_consistency`, and have the owner create the candidate
tag through the normal controlled process. This audit does not create the tag
and does not add a new checksum Gate condition.

## 7. Release Owner Sign-off

**Current status:** `OWNER_ACTION_REQUIRED`.

**Why it cannot PASS:** No named human has accepted the platform scope,
compatibility disposition, fault/property scope, ADR decisions, performance
bound, limitations, or residual risks. The existing sign-off file is
deliberately `PENDING` and `UNASSIGNED`.

**Exact Gate evidence required:** A completed
`M1_RELEASE_OWNER_SIGNOFF.md` naming the owner and date, binding the decision
to the exact commit and scope, listing accepted evidence and exceptions,
stating known limitations and risks, and recording `ACCEPTED` or `REJECTED`
with a human signature.

**Evidence already in the repository:** The unsigned sign-off template and
the pending acceptance registers in the compatibility, performance, release
matrix, and ADR documents.

**Missing:** Every human decision field and signature.

**Can a real test solve it?** No. Tests provide review inputs but cannot
provide accountability or acceptance.

**External dependency:** Yes, entirely human governance.

**Waiver:** No. The owner can sign an exception for a technical row where the
scope permits it, but cannot waive the requirement to sign the release
decision itself.

**Minimum plan if not waived:** After technical dispositions are complete, a
named Release Owner reviews the retained bundle and completes the sign-off
record. Until then the Gate remains blocked.

## MUST DO BEFORE RELEASE

The following actions must be completed or explicitly dispositioned before an
M1 release claim is possible:

1. Establish a named Release Owner and an explicit supported platform,
   filesystem, compatibility, and capacity scope.
2. Close Old Reader with the real historical-reader artifact and probes, or
   record the bounded compatibility exception described above.
3. Close FI-03 and the missing FI-07/FI-13/FI-14 rows for every platform and
   filesystem that remains in scope. A scope exclusion must be explicit.
4. Produce normative property records for every accepted row, including the
   required seed/case-count/cold-reopen metadata.
5. Have the owner accept or reject ADR-0015 and ADR-0016. Do not edit either
   ADR to `Accepted` without that decision.
6. Bind all accepted evidence to one clean release-candidate commit, verify
   the top-level `SHA256SUMS`, reconcile or label the selected
   `artifact-references.json` index, and pass artifact consistency on that
   exact commit.
7. Complete the human sign-off record with limitations, exceptions, risks,
   decision, and signature.
8. Run the complete quality gate and all M1 focused suites again on the exact
   candidate. A successful command set does not override an unresolved row.

## CAN BE WAIVED BY RELEASE OWNER

Only the following bounded exceptions are technically plausible, and each must
be written with the affected scope, residual risk, compensating control, owner,
date, and exact commit. A waiver does not change the underlying row to `PASS`:

- **Old Reader:** exclude v0.1 backward-read/migration support. Risk is loss of
  access to legacy repositories.
- **Fault Matrix FI-07/FI-13/FI-14:** exclude named platform/filesystem rows
  for which controlled resource or ACL evidence is unavailable. FI-03 may be
  excluded only by explicitly excluding external side-effect execution; it
  cannot be waived while that operation is supported.
- **Property Testing:** exclude named platform/filesystem rows from the
  supported matrix until their normative corpus is run. Risk is undetected
  platform-specific divergence.
- **ADR-0015:** accept a bounded capacity/performance scope with compensating
  operational limits and explicitly unmeasured metrics.

The following are not waiver candidates for an M1 release claim:

- ADR-0016 acceptance when projections remain in M1 scope;
- clean traceability and release-candidate provenance; and
- the human Release Owner sign-off itself.

## TECHNICAL WORK STILL REQUIRED

- Supply and probe a genuine historical reader, or preserve the compatibility
  exception as an explicit release-scope limitation.
- Add the external FI-03 execution schedule and complete the accepted-host
  FI-07/FI-13/FI-14 matrix using disposable resources.
- Run and retain the normative property corpus on every accepted row.
- Review the existing projection fixtures and FI-10 schedule against ADR-0016;
  add only evidence that is actually missing.
- Complete the three-run performance package for every accepted row, or
  prepare a compensating-bound proposal for owner decision.
- After the final evidence snapshot is known, create a clean candidate commit,
  regenerate traceability/checksums, reconcile or label the selected reference
  index and its self-entry, and rerun artifact consistency. This is release
  packaging hygiene, not a new M1 Gate condition.

No production-code change is required by this readiness audit. If a future
test exposes a Gate-critical defect, stop and record that defect before any
release decision; do not lower the test standard.

## HUMAN ACCEPTANCE REQUIRED

The Release Owner must personally decide and record:

- supported platforms and filesystems, including the fact that the hosted
  macOS row reports filesystem `unknown` and is not an APFS claim;
- Old Reader support or the bounded compatibility exception;
- FI-03/FI-07/FI-13/FI-14 scope and residual-risk disposition;
- property corpus scope and any excluded rows;
- ADR-0015 thresholds, capacity bounds, and exceptions;
- ADR-0016 acceptance or rejection;
- the exact candidate commit/version and evidence bundle; and
- the final `ACCEPTED` or `REJECTED` decision with name, date, and signature.

The implementation agent must not fill these fields or infer consent from green
CI output.

## FINAL RELEASE SEQUENCE

1. The Release Owner names the supported scope and assigns owners for
   compatibility, reliability, property, and performance evidence.
2. Obtain the old-reader artifact or sign the bounded compatibility exception.
3. Complete FI-03 and all in-scope resource/permission rows; retain raw
   platform, filesystem, error, exit, and cold-reopen evidence.
4. Complete the normative property corpus on every accepted row.
5. Review and record ADR-0015 and ADR-0016 decisions; preserve any explicit
   compensating bounds and risks.
6. Freeze the evidence snapshot in a clean candidate commit, regenerate
   traceability and nested checksums, and run `artifact_consistency`.
7. Run `cargo fmt --check`, `cargo check`, `cargo test`,
   `cargo clippy -- -D warnings`, the MSRV equivalents, and all M1 focused
   suites against that exact candidate.
8. The Release Owner completes `M1_RELEASE_OWNER_SIGNOFF.md`. Only the owner
   may then create a formal release tag and publish a release.
9. Mark M1 `PASS` only if every required row is evidenced or covered by a
   valid signed exception. Otherwise keep `M1_RELEASE_GATE = NOT_PASSED` and
   do not start M2.

## Release Owner: Exact Next Actions

If you are the current Release Owner, do these actions in order:

1. Name yourself in `M1_RELEASE_OWNER_SIGNOFF.md` and declare the exact M1
   support scope, especially the accepted filesystems and whether v0.1
   backward compatibility is required.
2. Choose one Old Reader disposition: supply the real v0.1 binary/fixture and
   probes, or sign the explicit compatibility exception.
3. Choose one fault/property disposition: run the missing FI-03/FI-07/FI-13/FI-14
   and cross-platform property evidence, or sign bounded exclusions for rows
   removed from the support scope.
4. Review the retained performance package and record the ADR-0015 threshold
   decision or a compensating capacity bound.
5. Review the projection fixtures and focused evidence, then record the
   ADR-0016 acceptance decision. Do not treat the existing test pass as that
   decision.
6. Ask the repository owner to freeze the final evidence in a clean candidate
   commit, regenerate traceability/checksums, and rerun the complete gates.
7. Review that exact candidate bundle, complete the sign-off with limitations,
   exceptions, risks, date, and signature, and only then authorize a formal
   release tag.

Until these actions are complete, the precise release conclusion is:

```text
OLD_READER = BLOCKED
FAULT_MATRIX = BLOCKED
PROPERTY_TESTING = NOT_PROVEN
ADR_0015_PERFORMANCE = NOT_PROVEN
ADR_0016_PROJECTION = OWNER_ACTION_REQUIRED (executable evidence retained; acceptance pending)
TRACEABILITY_RELEASE_CANDIDATE = BLOCKED
RELEASE_OWNER_SIGNOFF = BLOCKED
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```

## M1 Technical Evidence Closure Pass: Fault + Property (2026-08-28)

This section records only evidence actually executed in this closure pass. No
production code or ADR was changed. The release decision remains
`M1_RELEASE_GATE = BLOCKED / NOT_PASSED`.

### Environment probe

| Field | Observed result |
| --- | --- |
| Current host | Windows workspace, repository `D:\\pong` |
| Repository commit | `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` |
| Current filesystem | NTFS on `D:` |
| Available disk at probe time | `284,548,608,000` bytes free of `511,425,114,112` on `D:`; Windows volume reported healthy |
| Available memory at probe time | `3,741,152 KiB` free of `16,474,168 KiB` visible memory |
| Resource limits | No Linux `ulimit` context is available because no Linux distribution is installed; no Windows quota limit was asserted |
| Rust toolchain at probe time | `rustc 1.95.0`, `cargo 1.95.0`, host `x86_64-pc-windows-msvc` |
| Linux VM | `NOT_EXECUTED`; WSL has only stopped `docker-desktop`, no Linux distribution is installed |
| Docker | `NOT_EXECUTED`; Docker Desktop Linux daemon unavailable at `npipe:////./pipe/dockerDesktopLinuxEngine` |
| Native Linux evidence class | `UNAVAILABLE` for this pass; existing Docker records remain external Docker evidence |

### FI-03 / FI-07 / FI-13 / FI-14 disposition

| Scenario | Platform | Filesystem | Execution Method | External Dependency | Expected Result | Observed Result | Exit Code | Evidence Path | Evidence Class | Status |
| --- | --- | --- | --- | --- | --- | --- | ---: | --- | --- | --- |
| FI-03: external tool effect followed by abrupt termination before outcome append | Windows current host | NTFS | No independent provider/tool hook exists; only PT-10 generated unresolved schedules are available | Required external tool/provider harness | Tool effect must be reconciled as unknown or durable outcome after cold reopen | No real tool effect schedule executed or retained | N/A | `artifacts/m1-fault-matrix.json` | `SYNTHETIC` | `BLOCKED` |
| FI-07: CAS short write/quota during staging | Linux Docker `tmpfs` retained row | tmpfs | Retained host-resource execution from prior external run | Docker/Linux resource environment | `RESOURCE_EXHAUSTED`; no readable partial object; committed history and cold reopen survive | Retained Linux tmpfs resource records pass for the exercised row; Windows native disk-full/quota remains absent | 0 (retained row) | `artifacts/m1-fault-runs/linux-fi14-cas-2026-08-26.json` | `EXTERNAL` | `BLOCKED` |
| FI-13: permission revocation/read failure | Windows current host | NTFS | Dedicated scratch directory plus Windows `icacls` ACL revoke/restore | Windows ACL provider | `PERMISSION_DENIED`; committed data readable after restoration and cold reopen | Raw OS error `5`, Pong `PERMISSION_DENIED`; restoration, committed ref/CAS read, and cold reopen passed | 0 | `artifacts/m1-release-evidence/fault/windows-fi13-acl-closure-20260828.json` | `NATIVE` | `BLOCKED` (row PASS only) |
| FI-14: disk full during CAS/metadata/journal write | Linux Docker `tmpfs` retained rows | tmpfs | Retained controlled 64 MiB tmpfs exhaustion | Docker/Linux resource environment | `RESOURCE_EXHAUSTED`; committed history, staging cleanup, and cold reopen survive | CAS, metadata, and journal retained rows pass for Docker tmpfs; no current Linux VM or Windows native disk-full run | 0 (retained rows) | `artifacts/m1-fault-runs/linux-fi14-{cas,metadata,journal}-2026-08-26.json` | `EXTERNAL` | `BLOCKED` |

FI-03 remains blocked because a generated failpoint is not an external tool
effect. FI-07, FI-13, and FI-14 remain blocked at matrix level because the
required accepted platform/filesystem rows are incomplete. The new FI-13
record is a real Windows NTFS row only and does not widen the matrix.

### Property corpus

| Platform | Filesystem | Corpus Definition | Case Count | Command | Exit Code | Evidence Path | Retained Artifact | Status |
| --- | --- | --- | ---: | --- | ---: | --- | --- | --- |
| Windows current host | NTFS | PT-01..PT-08, PT-11, PT-12; repository-defined deterministic seeds; explicit normative override | 10,000 per property; 100,000 total | `PONG_PROPTEST_CASES=10000 CARGO_TARGET_DIR=D:\\pong\\target\\m1-closure-property-corpus-20260828 cargo test --locked --test property_corpus -- --nocapture` | 0 | `artifacts/m1-release-evidence/properties/windows-property-corpus-10000-20260828.json` | JSON + raw log retained | `PASS` for this Windows corpus scope |
| Linux current VM | N/A | Normative corpus not executed | 0 | Not executed | N/A | This checklist environment probe | No artifact created | `NOT_PROVEN` |
| Linux GitHub-hosted runner | ext4 | Existing Run `33145714975` standard suites; no retained per-property normative corpus record | Not recorded | Existing workflow commands | 0 for recorded suite commands | `artifacts/m1-release-evidence/platform/github-actions-run-33145714975/linux/` | Existing hosted artifact only | `NOT_PROVEN` |
| macOS GitHub-hosted runner | unknown | Existing Run `33145714975` standard suites; no retained per-property normative corpus record | Not recorded | Existing workflow commands | 0 for recorded suite commands | `artifacts/m1-release-evidence/platform/github-actions-run-33145714975/macos/` | Existing hosted artifact only | `NOT_PROVEN` |

The new Windows run does not prove PT-09, PT-10, PT-13, or PT-14; those
remain represented by their separate retained records. It also does not turn
the overall Property Testing blocker into `PASS`, because the normative
corpus is not retained for every accepted platform/filesystem row and owner
scope acceptance is pending.

### Performance confirmation

Existing `m1-perf-0.3` measurements and the prior Windows probe remain
measurement evidence. ADR-0015 is still `Proposed`; this pass did not accept
thresholds, create a new budget, or claim complete three-run coverage for
native Linux/macOS rows. Status: `OWNER_ACTION_REQUIRED`.

### Commands actually executed in this pass

```text
wsl.exe --status
wsl.exe --list --verbose
docker version --format '{{.Server.Version}}'
PONG_PROPTEST_CASES=10000 CARGO_TARGET_DIR=D:\\pong\\target\\m1-closure-property-corpus-20260828 cargo test --locked --test property_corpus -- --nocapture
PONG_HOST_FAULT_ROOT=D:\\pong\\.m1-fi13-closure-20260828 CARGO_TARGET_DIR=D:\\pong\\target\\m1-closure-fi13-20260828 cargo test --locked --test host_resource_faults fi_13_real_windows_acl_read_revocation_fails_closed_and_recovers -- --ignored --nocapture
```

### Evidence files created in this pass

```text
artifacts/m1-release-evidence/properties/windows-property-corpus-10000-20260828.json
artifacts/m1-release-evidence/properties/windows-property-corpus-10000-20260828.log
artifacts/m1-release-evidence/fault/windows-fi13-acl-closure-20260828.json
artifacts/m1-release-evidence/fault/windows-fi13-acl-closure-20260828-success.log
```

The release bundle checksum and reference manifests were regenerated after
adding the new records, and `artifact_consistency` passed both retained-log and
bundle-wide checks in this pass. The bundle is still not a release candidate:
the working tree is dirty, and the manifests must be regenerated again after
the final clean candidate commit is frozen.

## M1 Linux Native Evidence Execution Pass (2026-08-28)

This pass used the dedicated SSH Linux VM `rtc-node-1`; these results are
`NATIVE` evidence and are Linux-native VM executions, not Docker evidence. The source was transferred
as a read-only Git bundle and checked out at the same candidate commit as the
Windows workspace.

### Linux environment

| Field | Observed result |
| --- | --- |
| Hostname | `rtc-node-1` |
| OS / kernel | Debian GNU/Linux 13 (trixie); Linux `6.12.101+deb13-amd64` x86_64 |
| Root filesystem | `/dev/sda1`, `ext4`; dedicated FI-14 mount is `tmpfs` 64 MiB |
| CPU / memory | 4 CPUs; 1.9 GiB total, about 1.2 GiB available at probe |
| Rust / Cargo / tools | Rust/Cargo/Rustfmt/Clippy `1.85.0` system packages; Git `2.47.3` |
| MSRV | Project requires `1.78.0`; `1.78.0` distribution could not be installed, so no MSRV PASS is claimed |
| Repository | `/root/pong-linux-native-6cb62fb`, detached clean checkout at `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` |
| Execution identity | `root`; Linux VM, not a physical Linux server claim |

### Linux base gates

| Command | Exit code | Status | Evidence |
| --- | ---: | --- | --- |
| `cargo fmt --all -- --check` | 0 | `PASS` with Rust 1.85 | `platform/linux-native-vm-6cb62fb-20260828/fmt.log` |
| `cargo check --locked --offline` | 0 | `PASS` with Rust 1.85 | `platform/linux-native-vm-6cb62fb-20260828/check.log` |
| `cargo test --all --locked --offline` | 0 | `PASS` with Rust 1.85; ignored tests are not promoted | `platform/linux-native-vm-6cb62fb-20260828/test.log` |
| `cargo clippy --all-targets --all-features --locked --offline -- -D warnings` | 0 | `PASS` with Rust 1.85 | `platform/linux-native-vm-6cb62fb-20260828/clippy.log` |

The final successful commands used the lockfile and a copied registry cache.
This does not provide the project's Rust 1.78 MSRV evidence.

### Linux FI-03 / FI-07 / FI-13 / FI-14

| Scenario | Platform | Filesystem | Execution Method | External Dependency | Expected Result | Observed Result | Exit Code | Evidence Path | Evidence Class | Status |
| --- | --- | --- | --- | --- | --- | --- | ---: | --- | --- | --- |
| FI-03 external tool effect then abrupt termination | Linux VM | ext4 | No independent external-tool test entry exists; only generated PT-10 schedules | Required provider/tool harness | Reconcile effect as unknown or durable after cold reopen | Not executed; no external effect claimed | N/A | `artifacts/m1-fault-matrix.json` | `SYNTHETIC` | `BLOCKED` |
| FI-07 CAS short write/quota | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Existing ignored host-resource CAS schedule | `tmpfs`, executed as `root` | `RESOURCE_EXHAUSTED`; no readable partial object; committed history survives | Real `errno=28`, Pong `RESOURCE_EXHAUSTED`; cold-reopen assertions passed | 0 | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` | `NATIVE` | `PASS` for Linux tmpfs row only |
| FI-13 permission/ACL denial | Linux VM | ext4 | No Linux chmod/chown/ACL test entry exists | Future Linux permission harness required | Permission denial must fail closed and recover after restoration | Not executed; no Linux permission evidence invented | N/A | `artifacts/m1-release-evidence/fault/linux-fi13-not-executed-20260828.json` | `UNAVAILABLE` | `NOT_EXECUTED` |
| FI-14 CAS ENOSPC | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Existing ignored host-resource schedule | `tmpfs`, executed as `root` | `RESOURCE_EXHAUSTED`; committed history and cold reopen survive | `errno=28`, Pong `RESOURCE_EXHAUSTED`, exit `0` | 0 | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` | `NATIVE` | `PASS` for Linux tmpfs row only |
| FI-14 metadata ENOSPC | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Existing ignored host-resource schedule | `tmpfs`, executed as `root` | `RESOURCE_EXHAUSTED`; committed history and cold reopen survive | `errno=28`, Pong `RESOURCE_EXHAUSTED`, exit `0` | 0 | `artifacts/m1-release-evidence/fault/linux-native-fi14-metadata-20260828.json` | `NATIVE` | `PASS` for Linux tmpfs row only |
| FI-14 journal ENOSPC | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Existing ignored host-resource schedule | `tmpfs`, executed as `root` | `RESOURCE_EXHAUSTED`; committed history and cold reopen survive | `errno=28`, Pong `RESOURCE_EXHAUSTED`, exit `0` | 0 | `artifacts/m1-release-evidence/fault/linux-native-fi14-journal-20260828.json` | `NATIVE` | `PASS` for Linux tmpfs row only |

The Fault Matrix blocker remains `BLOCKED`: FI-03 lacks a real provider effect
schedule, FI-13 has no Linux execution path, and the complete accepted matrix
is not established.

### Linux property corpus

| Platform | Filesystem | Corpus / property | Cases | Exit | Evidence | Status |
| --- | --- | --- | ---: | ---: | --- | --- |
| Linux VM `rtc-node-1` | ext4 | PT-01..PT-08, PT-11, PT-12 | 10,000 each; 100,000 total | 0 | `properties/linux-native-vm-property-corpus-10000-20260828.json` + raw log | `PASS` for Linux Rust 1.85 row |
| Linux VM `rtc-node-1` | ext4 | PT-09 recovery convergence | 10,000 | 0 | `properties/linux-native-vm-pt09-10000-20260828.json` + raw log | `PASS` for Linux Rust 1.85 row |
| Linux VM `rtc-node-1` | ext4 | PT-10 unresolved crash schedules | 10,000 | 0 | `properties/linux-native-vm-pt10-10000-20260828.json` + raw log | `PASS` for Linux Rust 1.85 row |
| Linux VM `rtc-node-1` | ext4 | PT-14 migration failpoints | 10,008 across 9 failpoints | 0 | `properties/linux-native-vm-pt14-10000-20260828.json` + raw log | `PASS` for Linux Rust 1.85 row |

No Linux normative corpus was run with Rust 1.78 because that toolchain was
unavailable. The overall Property Testing blocker remains `NOT_PROVEN` until
the accepted rows and toolchain scope are explicitly defined and retained.

### Linux evidence and consistency

Evidence was added under `artifacts/m1-release-evidence/platform/linux-native-vm-6cb62fb-20260828/`,
`artifacts/m1-release-evidence/properties/`, and `artifacts/m1-release-evidence/fault/`.
The bundle references and `SHA256SUMS` were regenerated, and
`cargo test --locked --test artifact_consistency -- --nocapture` passed both
consistency tests. This remains an uncommitted evidence snapshot, not a release
candidate.

## M1 Technical Evidence Closure: Fault + Property (2026-08-29)

This follow-up pass was limited to the Fault Matrix and Property Testing
blockers. No production code, ADR-0015/ADR-0016 status, release tag, waiver,
or final gate report was changed or created. Existing Linux-native records
remain authoritative for the same candidate commit; completed 10,000-case
property runs and completed disposable-resource schedules were not rerun in
this pass, so no duplicate PASS artifact was created.

### Linux VM read-only confirmation

| Field | Observed result |
| --- | --- |
| Host / target | `rtc-node-1` / `root@192.168.59.128` |
| Distribution / kernel | Debian GNU/Linux 13 (trixie); Linux `6.12.101+deb13-amd64` x86_64 |
| Root filesystem | `/dev/sda1`, `ext4` (`findmnt /` and `df -T /`) |
| Resources at probe | 4 CPUs; 1.9 GiB total memory, about 1.1 GiB available; swap 2.0 GiB |
| Resource limits | open files `1024`; max user processes `7517`; file size unlimited |
| Toolchain | Rust/Cargo `1.85.0`; Git `2.47.3`; project MSRV `1.78.0` unavailable in VM |
| Repository | `/root/pong-linux-native-6cb62fb`; detached clean `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` |
| Execution identity | `root`; Linux VM evidence, not a physical Linux server claim |

The repository commit matches the Windows candidate. The VM's `stat -f` output
reports the generic `ext2/ext3` family while `findmnt` and `df -T` report the
mounted filesystem as `ext4`; the evidence records the filesystem using the
mount tools' `ext4` result. The dedicated `tmpfs` used by the retained FI-14
records is a separate 64 MiB test mount.

### FI-03 / FI-07 / FI-13 / FI-14 confirmation

| Scenario | Platform | Filesystem | Execution Method | External Dependency | Expected Result | Observed Result | Exit Code | Evidence Path | Evidence Class | Status |
| --- | --- | --- | --- | --- | --- | --- | ---: | --- | --- | --- |
| FI-03: external tool effect then abrupt termination before outcome append | Linux VM | ext4 | Read-only source/test-entry audit; `property_recovery_migration --list`; no provider hook | Real external provider/tool harness | Unknown until provider/workspace reconciliation; blind retry blocked | No external effect executed; retained PT-10 schedules are synthetic groundwork only | N/A | `artifacts/m1-fault-matrix.json` | `SYNTHETIC` | `BLOCKED` |
| FI-07: CAS short write/quota | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Retained ignored host-resource CAS schedule; not rerun | Disposable Linux resource mount | Partial object unreachable; committed history and cold reopen survive | Retained Linux-native record reports `errno=28`, `RESOURCE_EXHAUSTED`, cleanup, and cold-reopen success; scoped to tmpfs | 0 (retained) | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` | `NATIVE` | `PASS` for this row only; matrix `BLOCKED` |
| FI-13: permission revocation/read failure | Linux VM | ext4 | `host_resource_faults --list` exposes only the Windows `icacls` test; no Linux chmod/chown/ACL entry | Future Linux permission harness or Owner scope decision | Permission/security status fails closed and recovers after restoration | Not executed; no Linux permission evidence invented | N/A | `artifacts/m1-release-evidence/fault/linux-fi13-not-executed-20260828.json` | `UNAVAILABLE` | `NOT_PROVEN` |
| FI-14: CAS ENOSPC | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Retained ignored host-resource schedule; not rerun | Disposable `tmpfs` mount | `RESOURCE_EXHAUSTED`; committed history and cold reopen survive | Retained `errno=28`, Pong `RESOURCE_EXHAUSTED`, exit `0`, and cold-reopen assertions | 0 (retained) | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` | `NATIVE` | `PASS` for this row only; matrix `BLOCKED` |
| FI-14: metadata ENOSPC | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Retained ignored host-resource schedule; not rerun | Disposable `tmpfs` mount | `RESOURCE_EXHAUSTED`; committed history and cold reopen survive | Retained `errno=28`, Pong `RESOURCE_EXHAUSTED`, exit `0`, and cold-reopen assertions | 0 (retained) | `artifacts/m1-release-evidence/fault/linux-native-fi14-metadata-20260828.json` | `NATIVE` | `PASS` for this row only; matrix `BLOCKED` |
| FI-14: journal ENOSPC | Linux VM `rtc-node-1` | dedicated 64 MiB `tmpfs` | Retained ignored host-resource schedule; not rerun | Disposable `tmpfs` mount | `RESOURCE_EXHAUSTED`; committed history and cold reopen survive | Retained `errno=28`, Pong `RESOURCE_EXHAUSTED`, exit `0`, and cold-reopen assertions | 0 (retained) | `artifacts/m1-release-evidence/fault/linux-native-fi14-journal-20260828.json` | `NATIVE` | `PASS` for this row only; matrix `BLOCKED` |

FI-07/FI-14 evidence is limited to the exercised `tmpfs` resource model and
does not claim Windows NTFS quota behavior or generic Linux ext4 behavior.
In this table, `NATIVE` is the required evidence-class label; the Platform
column identifies that these rows are Linux-native VM executions and not
Docker or physical-host evidence.
FI-03 remains blocked because a generated failpoint is not a real provider
effect. FI-13 remains unexecuted on Linux because the repository has no Linux
permission test entry; adding one would be a separate scope decision, not an
inferred result.

### Property corpus confirmation

| Platform | Filesystem | Corpus Definition | Case Count | Command | Exit Code | Evidence Path | Retained Artifact | Status |
| --- | --- | --- | ---: | --- | ---: | --- | --- | --- |
| Windows x86_64 current host | NTFS | Retained normative PT-01..PT-12 subset; PT-09/PT-10/PT-14 separate records | 10,000/property where recorded; PT-14 10,008 across 9 failpoints | Existing `PONG_PROPTEST_CASES=10000` runs | 0 (retained) | `artifacts/m1-release-evidence/properties/windows-property-corpus-10000-20260828.json` and related records | JSON and raw logs | `PASS` for recorded rows; overall `NOT_PROVEN` |
| Linux VM `rtc-node-1` | ext4 | Retained normative PT-01..PT-12 subset plus PT-09/PT-10/PT-14 records | PT-01..08/11/12: 10,000 each; PT-09/PT-10: 10,000; PT-14: 10,008 | Existing `PONG_PROPTEST_CASES=10000` runs on Rust 1.85 | 0 (retained) | `artifacts/m1-release-evidence/properties/linux-native-vm-*.json` and matching logs | JSON and raw logs; MSRV 1.78 unavailable | `PASS` for Linux VM Rust 1.85 rows; overall `NOT_PROVEN` |
| Linux GitHub-hosted runner | ext4 | Run `33145714975` standard property/focused suites; no retained per-property normative corpus | Not recorded per property | Existing workflow commands | 0 (retained) | `artifacts/m1-release-evidence/platform/github-actions-run-33145714975/linux/` | Hosted artifact | `NOT_PROVEN` for normative corpus claim |
| macOS GitHub-hosted runner | `unknown` | Run `33145714975` standard property/focused suites; no retained per-property normative corpus | Not recorded per property | Existing workflow commands | 0 (retained) | `artifacts/m1-release-evidence/platform/github-actions-run-33145714975/macos/` | Hosted artifact | `NOT_PROVEN` for normative corpus claim |

PT-13 is the fixed event/projection fixture suite, not a generated 10,000-case
property; its release acceptance remains coupled to ADR-0016. No case count is
inferred from hosted macOS logs, and no physical Mac evidence is claimed. The
overall Property Testing blocker remains `NOT_PROVEN`.

### Performance confirmation

The retained `m1-perf-0.3` package and Windows/Linux-overlay raw runs provide
measurement evidence. No performance command was run in this pass, no
threshold was changed, and ADR-0015 remains `OWNER_ACTION_REQUIRED` /
`Proposed`; measurement does not equal acceptance.

### Commands actually executed in this pass

These commands were read-only environment, repository, and test-entry probes;
the `--list` commands enumerate tests and do not execute property or fault
cases. All returned exit code `0`. In the block below, `ssh ...` abbreviates
the connection options and identity shown on the first line; the remote
command portion is recorded for each probe.

```text
ssh -o BatchMode=yes -o ConnectTimeout=5 -i "$HOME/.ssh/pong_linux_ed25519" -o IdentitiesOnly=yes root@192.168.59.128 hostname
ssh ... root@192.168.59.128 uname -a
ssh ... root@192.168.59.128 cat /etc/os-release
ssh ... root@192.168.59.128 findmnt /
ssh ... root@192.168.59.128 df -T /
ssh ... root@192.168.59.128 free -h
ssh ... root@192.168.59.128 nproc
ssh ... root@192.168.59.128 rustc --version
ssh ... root@192.168.59.128 cargo --version
ssh ... root@192.168.59.128 git --version
ssh ... root@192.168.59.128 'cd /root/pong-linux-native-6cb62fb && git rev-parse HEAD && git status --porcelain=v1 && df -T . /tmp && free -h && ulimit -n && ulimit -u'
ssh ... root@192.168.59.128 'cd /root/pong-linux-native-6cb62fb && cargo test --locked --offline --test host_resource_faults -- --list'
ssh ... root@192.168.59.128 'cd /root/pong-linux-native-6cb62fb && cargo test --locked --offline --test property_corpus -- --list'
ssh ... root@192.168.59.128 'cd /root/pong-linux-native-6cb62fb && cargo test --locked --offline --test property_recovery_migration -- --list'
ssh ... root@192.168.59.128 'cd /root/pong-linux-native-6cb62fb && rg/grep source entries for FI-13/FI-14 and external/provider hooks'
```

No new evidence files were created in this pass. Existing retained artifacts
were not overwritten, failed evidence was not deleted, and no artifact
manifest/checksum was changed.

### Closure result

```text
FI-03 = BLOCKED
FI-07 = PASS for retained Linux tmpfs row only; FAULT_MATRIX = BLOCKED
FI-13 (Linux) = NOT_PROVEN (NOT_EXECUTED)
FI-14 = PASS for retained Linux tmpfs CAS/metadata/journal rows only; FAULT_MATRIX = BLOCKED
PROPERTY_TESTING = NOT_PROVEN
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```

This pass stops at technical evidence closure. It does not enter M2 or perform
release actions.

## M1 Final Technical Gap Closure: Linux FI-13 (2026-08-29)

This closure pass addressed only the Linux FI-13 permission row identified by
the frozen M1 scope. No production code, ADR-0015, ADR-0016, scope definition,
release tag, waiver, or final gate report was changed or created.

### Environment

| Field | Observed result |
| --- | --- |
| Host / target | `rtc-node-1` / Linux VM |
| Distribution / kernel | Debian GNU/Linux 13 (trixie); Linux `6.12.101+deb13-amd64` x86_64 |
| Filesystem | `/dev/sda1` mounted as `ext4` at `/` |
| Repository | `/root/pong-linux-native-6cb62fb` |
| Commit | `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` |
| Execution identity | `uid=65534(nobody) gid=65534(nogroup)` |
| Evidence class | `NATIVE` Linux VM evidence; not a physical-host claim |

### FI-13 Linux row

| Scenario | Platform | Filesystem | Execution Method | External Dependency | Expected Result | Observed Result | Exit Code | Evidence Path | Evidence Class | Status |
| --- | --- | --- | --- | --- | --- | --- | ---: | --- | --- | --- |
| POSIX permission revocation/read failure | Linux VM `rtc-node-1` | ext4 | `runuser -u nobody` with mode-bit revocation on a dedicated scratch repository | None beyond the Linux VM filesystem and `runuser` | OS permission failure maps to `PERMISSION_DENIED`; repository remains valid; restored permissions permit cold reopen and read | `chmod 0644 -> 0200` produced raw `errno=13` (`EACCES`); Pong returned `PERMISSION_DENIED`; permission restore, committed-ref/CAS read, and cold reopen succeeded | 0 | `artifacts/m1-release-evidence/fault/linux-fi13-posix-closure-20260829.json` and `.log` | `NATIVE` | `PASS` for the Linux ext4 row |

The retained JSON includes the command, timestamp, commit, UID/GID, mode bits,
raw errno, Pong error, recovery and cold-reopen results. The raw log digest is
`C4A11E7D0459BE77EA5388F0898DDD947756E039A942C9BFC11714CC1FC42198`.

### Artifact consistency

`cargo test --locked --test artifact_consistency -- --nocapture` passed both
consistency tests after the new JSON/log were added. The top-level
`artifacts/m1-release-evidence/SHA256SUMS` and `artifact-references.json` now
cover the new records, and the retained source fault-matrix copy was refreshed
to the same FI-13 disposition. Existing failure evidence was not deleted or
overwritten.

### Disposition after this pass

| Item | Status | Reason |
| --- | --- | --- |
| Linux FI-13 ext4 row | `PASS` | Real POSIX mode-bit denial and recovery observed on Linux VM with exit code 0. |
| FI-03 | `BLOCKED` | No real external-provider effect schedule was executed. |
| FI-07 | `PASS` for retained Linux tmpfs row only | Complete accepted-platform matrix is still not established. |
| FI-14 | `PASS` for retained Linux tmpfs rows only | Windows and other accepted resource rows remain absent. |
| Fault Matrix overall | `BLOCKED` | FI-03 and complete accepted-row coverage remain unresolved. |
| Property Testing overall | `NOT_PROVEN` | Existing Linux corpus is retained; cross-platform normative acceptance remains incomplete. |
| M1 Release Gate | `BLOCKED / NOT_PASSED` | Old Reader, fault/property scope, ADR acceptance, traceability, and sign-off remain open. |

This pass is complete and stops here; it does not enter M2 or perform release
actions.
