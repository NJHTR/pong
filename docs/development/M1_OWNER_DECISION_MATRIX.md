# M1 Owner Decision Matrix

**Prepared:** 2026-08-29  
**Candidate commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Native evidence workflow:** `33145714975`  
**Decision authority:** Release Owner  
**Current gate:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`

This document separates facts already evidenced from work that is conditional
on a Release Owner decision. It does not accept an ADR, sign a waiver, create a
tag, freeze a release candidate, or change the M1 Gate.

## Decision Matrix

| Item | Current Status | Evidence | Technical Work Remaining | Owner Decision | Waiver Possible | Recommended Disposition |
|---|---|---|---|---|---|---|
| FI-03 | `BLOCKED` while external effects are claimed; `DEFERRED_PENDING_OWNER_DECISION` otherwise | `artifacts/m1-fault-matrix.json`; PT-10 unresolved-intent schedules are `SYNTHETIC` only; no independent provider hook | If retained in M1: define the smallest provider execution/reconciliation interface, run a real side effect, terminate after effect/before outcome append, cold-reopen and reconcile, retain `EXTERNAL` evidence. No implementation is started in this preparation pass. | Choose **A**: keep external side effects as an M1 MUST, or **B**: explicitly exclude them and defer FI-03 to M2+. | `CONDITIONAL`: only by excluding external-side-effect execution from M1. It is not a waiver while that capability is claimed. | **B is recommended** for the first durable-core release: record `No M1 claim for external side-effect reconciliation; FI-03 deferred to M2+`. |
| FI-07 | `BLOCKED` at matrix level; Linux `tmpfs` resource row `PASS` only | Linux VM `rtc-node-1`, dedicated 64 MiB `tmpfs`; real `errno=28`, `RESOURCE_EXHAUSTED`, staging cleanup and cold reopen in retained CAS evidence. Synthetic short-write/quota tests cover code paths only. | **Option A:** execute safe native rows for every resource/filesystem model Owner accepts, including ext4/NTFS if they are promised. **Option B:** no new test for excluded models, but the release contract must state the bounded resource model. | Decide whether M1 promises generic resource-exhaustion semantics on Linux ext4 and Windows NTFS, or only the named Linux `tmpfs` qualification model. | `YES, CONDITIONAL`: exclude named unmeasured resource models and record residual risk. | **Option B is recommended** unless the product explicitly needs generic ext4/NTFS quota guarantees. Required wording: `Pong M1 does not promise all filesystem resource-exhaustion semantics; the measured resource boundary is the dedicated Linux tmpfs model.` |
| FI-13 | Row-level `PASS` for Windows NTFS and Linux ext4; overall release acceptance pending | Windows NTFS ACL evidence: raw error 5 -> `PERMISSION_DENIED`; Linux ext4 POSIX evidence: raw errno 13 -> `PERMISSION_DENIED`, restoration and cold reopen pass. `linux-fi13-posix-closure-20260829.json/.log` retained as `NATIVE`. | None for the proposed Windows/Linux rows. A new technical run is needed only if the Owner makes macOS a local supported filesystem rather than qualification-only. | Decide whether macOS permission-fault semantics are outside M1 because the runner filesystem is `unknown` and qualification-only. | `YES` only as a scope exclusion for macOS; not needed for the already evidenced Windows/Linux rows. | Record `macOS FI-13 = NOT_APPLICABLE / QUALIFICATION_ONLY`; do not call the runner APFS or physical Mac. |
| FI-14 | `BLOCKED` at matrix level; Linux `tmpfs` CAS/metadata/journal rows `PASS` only | Retained Linux-native VM records for CAS, metadata, and journal `ENOSPC`; exit `0`, `errno=28`, `RESOURCE_EXHAUSTED`, committed-history preservation and cold reopen. | **Option A:** add safe native ext4/NTFS CAS, metadata, and journal resource schedules for every model promised. **Option B:** exclude those models from the M1 resource guarantee and amend the release scope record. | Decide whether Windows NTFS, Linux ext4, or macOS `unknown` resource behavior is part of the M1 contract. | `YES, CONDITIONAL`: exclude named models; underlying missing rows remain unverified. | **Option B is recommended** for bounded M1; keep Linux `tmpfs` as a qualification record, not a generic filesystem promise. |
| Property | `NOT_PROVEN` pending scope/policy acceptance | Windows and Linux VM retained 10,000-case/property records; macOS GitHub-hosted standard/focused suites; no inferred macOS 10,000 corpus. `M1_PROPERTY_SCOPE_DECISION.md` defines the pending policy. | **Policy A:** no new large run beyond required qualification if Owner accepts one canonical corpus. **Policy B:** run and retain 10,000 cases for every accepted platform. **Policy C:** document another bounded policy and its evidence requirements. | Choose the normative corpus policy and exact accepted platform rows. | `YES, CONDITIONAL`: exclude named platform/property rows with residual-risk statement; this does not relabel them `PASS`. | **Policy A recommended:** canonical deterministic 10,000-case corpus plus cross-platform qualification; do not multiply 10,000 cases by platform unless explicitly chosen. |
| Old Reader | `BLOCKED` | `artifacts/m1-release-evidence/compatibility/old-reader-audit.json/.log` report no independent v0.1 source, tag, binary, fixture, or probe. | Only **Option A** requires technical work: obtain a real historical binary/fixture, immutable hash, read/mutation/cold-reopen probes and retained output. Options B/C require no fake reader. | Choose **A:** provide the real historical reader; **B:** sign a compatibility waiver; or **C:** define unreleased pre-M1 formats as out of scope. | `YES, CONDITIONAL`: B or C may exclude v0.1 backward compatibility. | **C recommended** if M1 is the first formal release baseline, with the explicit statement: `No backwards compatibility guarantee for unreleased pre-M1 repository formats.` |
| ADR-0015 | `OWNER_ACTION_REQUIRED` | Measurement exists in `m1-perf-0.3` and retained Windows/Linux-overlay runs; ADR remains `Proposed`; no accepted threshold. | If hard-gate policy is chosen, complete required measurements for the accepted scope and preserve thresholds. If baseline-only is chosen, no new production code is needed. | Choose **A:** performance is an informational M1 baseline with future capacity qualification deferred; or **B:** adopt explicit thresholds as a hard release gate. | `YES, CONDITIONAL`: accept a bounded baseline/capacity scope with compensating limits. | **A recommended** for M1 durable-core scope; record that performance is informational and capacity qualification is deferred. ADR itself remains unchanged until formally accepted. |
| ADR-0016 | `OWNER_ACTION_REQUIRED` | Projection implementation and focused evidence pass for the recorded scope: `tests/event_projection.rs`, `tests/pt13_fi10.rs`, retained Run `33145714975` Linux/macOS focused artifacts. ADR status remains `Proposed`. | No coding gap identified for the recorded scope. Additional work is needed only if Owner rejects the evidence or changes the projection scope. | Accept or reject the proposed generation-bound projection contract. | `NO` while projections remain in M1. Removing projections would be a scope change, not a waiver. | Review the retained fixtures and record **ACCEPTED** or **REJECTED**; do not edit the ADR automatically. |
| Traceability | `BLOCKED` | `traceability/release-traceability.json/.log` bind commit `6cb62fb...` and workflow `33145714975`; bundle checksums and artifact consistency currently pass. | After decisions: freeze one final evidence snapshot, regenerate selected references/checksums, rerun consistency against the exact commit, and create a clean candidate through the controlled owner process. | Approve the final scope and authorize candidate freeze/versioning. | `NO` for a release-candidate provenance claim. | Defer candidate freeze until all decisions and sign-off inputs are recorded; no tag is created here. |
| Release Owner | `OWNER_ACTION_REQUIRED` | `M1_RELEASE_OWNER_SIGNOFF.md` is intentionally `UNASSIGNED`/`PENDING`; no accepted scope, exception, decision, date, or signature exists. | Codex cannot perform this governance action. Technical packaging can resume after the Owner decisions. | Name the owner, accepted scope, evidence, exceptions, risks, decision, date, and signature. | `NO`: the sign-off is the governance control itself. | Complete the sign-off only after the final evidence snapshot and all chosen exceptions are explicit. |

## FI-03 Decision Detail

The phrase **external side effect** means a real provider/tool action that can
change state outside Pong, followed by abrupt termination before the outcome is
appended. The current M1 Core has no provider execution interface. PT-10 only
proves local unknown preservation. It must not be described as external
provider evidence.

| Choice | Scope effect | Cost | Risk |
|---|---|---|---|
| A. Keep FI-03 as an M1 MUST | M1 claims safe handling of external side effects | New provider contract, disposable effect harness, process interruption and reconciliation evidence | Until complete, blind retry and external-state ambiguity remain unverified |
| B. Defer FI-03 to M2+ | M1 claims durable local core only; external side effects are excluded | Owner scope/sign-off update only in this phase | M1 users cannot rely on Pong to reconcile external effects; M2+ must define the boundary |

## FI-07 / FI-14 Scope Boundary

The existing evidence proves the dedicated Linux `tmpfs` resource model. It
does not prove generic Linux ext4, Windows NTFS quota behavior, or the unknown
filesystem behind the GitHub-hosted macOS runner. Choosing the bounded option
is a product-scope decision, not a test relabeling.

## Property Policy Choices

| Policy | Meaning | Additional cost |
|---|---|---|
| A. Canonical corpus + qualification | One retained deterministic 10,000-case corpus per generated property, plus selected cross-platform qualification | Lowest; matches the current proposed interpretation |
| B. Full corpus per accepted platform | 10,000 cases per property on every accepted platform | Highest; requires additional macOS and any newly accepted rows |
| C. Other bounded policy | Owner defines the exact corpus, accepted rows, and residual risk | Must be documented before release; cannot be inferred by Codex |

## Current Release Boundary

The following facts remain unchanged regardless of the Owner's choice:

- Windows NTFS and Linux ext4 FI-13 permission rows have real retained
  evidence; macOS has no filesystem-specific permission claim.
- Linux `tmpfs` FI-07/FI-14 rows are qualification evidence only.
- No historical v0.1 reader was found.
- ADR-0015 and ADR-0016 remain `Proposed`.
- The working tree is dirty, no release tag exists, and sign-off is pending.
- `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`.

This matrix is decision preparation, not release approval.
