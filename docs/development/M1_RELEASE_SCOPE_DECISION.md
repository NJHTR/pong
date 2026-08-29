# M1 Release Scope Decision

**Prepared:** 2026-08-28  
**Audited branch:** `dev`  
**Audited commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Native evidence workflow:** `33145714975`  
**Working tree:** `DIRTY`  
**Release tag:** none  
**M1_RELEASE_GATE:** `BLOCKED / NOT_PASSED`

> **Final acceptance overlay (2026-08-29):** Release Owner `NJHTR` accepted
> the bounded platform, fault, property, compatibility, performance, and
> projection decisions. This document's proposed/pending tables are retained
> as historical preparation. Candidate freeze is still pending and no release
> tag exists; see `M1_FINAL_GATE_REPORT.md`.

This document records the boundary between technical requirements and scope or
governance decisions. It does not change the M1 Gate, create a tag, commit the
working tree, accept an ADR, or sign a waiver.

## Proposed M1 Supported Platform Scope

The release-scope proposal prepared for Release Owner review contains these
three rows:

1. Windows x86_64 on NTFS.
2. Linux x86_64 on ext4.
3. A GitHub-hosted macOS runner (arm64 in Run `33145714975`), with the
   repository filesystem recorded as `unknown`.

This is a **PROPOSED** scope, not an accepted support claim. The third row must
be described as a GitHub-hosted macOS runner; the retained evidence does not
establish a physical Mac or APFS filesystem. Linux-native Debian 13 VM evidence
(`Linux 6.12.101+deb13-amd64`, ext4) is supporting evidence for the Linux row,
but it ran Rust 1.85 rather than the Rust 1.78 MSRV. The Release Owner must
freeze the accepted rows and record the decision in
`M1_RELEASE_OWNER_SIGNOFF.md`.

## M1 Release Characterization

M1 is proposed as a **durability, correctness, and recovery-focused release
baseline** for the accepted rows above. It is not, by itself, universal
platform certification, external-provider certification, a historical
pre-release compatibility guarantee, or a performance-capacity certification.
Those boundaries narrow product claims; they do not turn missing evidence into
`PASS` and require Owner acceptance where they affect a Gate row.

## Scope Decision Package

The following package is the proposed interpretation to take to the Release
Owner. `PROPOSED` and `OWNER_ACTION_REQUIRED` are deliberate: none of these
rows is accepted by Codex.

| Scope dimension | Proposed disposition | Current evidence boundary | Effective status |
| --- | --- | --- | --- |
| Platform | Windows x86_64/NTFS; Linux x86_64/ext4; GitHub-hosted macOS runner with filesystem `unknown` | Run `33145714975` executable rows; Debian 13 VM is Linux-native supporting evidence on Rust 1.85 | `PROPOSED / OWNER_ACTION_REQUIRED` |
| Faults | FI-01/02/04/05/06/08/09/10/11/12 required; FI-07/FI-14 where the accepted resource model applies; FI-13 required for accepted local rows (Windows evidence retained, Linux row missing); FI-03 proposed deferred | Synthetic and retained host/resource rows only; no external-provider FI-03 schedule | `BLOCKED` until scope is accepted |
| Properties | One canonical normative corpus of 10,000 cases/property; cross-platform smoke/selected qualification without inventing per-platform 10,000 claims | Windows/Linux retained high-count records; hosted macOS quality/focused evidence lacks a complete retained per-property corpus | `NOT_PROVEN` |
| Old Reader | Proposed `WAIVED / NOT_APPLICABLE` for unreleased pre-M1 formats, with no backward-compatibility guarantee | No independent v0.1 reader artifact or probe exists | `BLOCKED` until a signed compatibility decision is recorded |
| Performance | `m1-perf-0.3` is a measurement baseline; no threshold is accepted automatically | Repeated Windows and Linux-overlay measurements; native ext4/macOS repetitions remain incomplete | `OWNER_ACTION_REQUIRED` (ADR-0015 Proposed) |
| Projection | Keep the generation-bound projection slice in M1 and present ADR-0016 for acceptance | PT-13/FI-10, event/projection, migration, and hosted focused evidence retained | `OWNER_ACTION_REQUIRED` (ADR-0016 Proposed) |

The property proposal does not rewrite the Gate's PT-01 through PT-14
invariants. If the Owner requires a normative corpus on every accepted row,
the missing row-level records become technical work; otherwise the Owner must
record the bounded qualification policy and its residual risk explicitly.

### Compatibility and external-effect proposals

- **Old Reader:** Because no independently released v0.1 reader exists in the
  checked history, treating M1 as the first formal compatibility baseline is a
  proposed Owner policy, not an existing repository fact. Under that proposal,
  the disposition is `WAIVED / NOT_APPLICABLE (PENDING OWNER ACCEPTANCE)`.
  This is not a signed waiver. Until the Owner records it, the machine status
  remains `BLOCKED`.
  The explicit risk is that pre-M1 unreleased/internal repository formats have
  no backward-compatibility guarantee.
- **FI-03:** `DEFERRED (PENDING OWNER ACCEPTANCE)` is proposed because M1 Core
  has no independent external provider/tool execution hook. Synthetic unknown-
  effect tests remain implementation groundwork and are not external-effect
  evidence. This deferral is valid only if the M1 claim explicitly excludes
  external provider side effects; otherwise FI-03 remains a required blocker.
- **Linux FI-13:** Linux POSIX/ACL qualification is required if Linux remains
  an accepted local platform. No Linux permission test is added by this
  decision. The row may be recorded `NOT_APPLICABLE FOR M1` only if the Owner
  removes Linux from the supported M1 platform scope; otherwise it remains a
  missing required row.

## Required for M1

The current Gate requires all MUST-PASS contract, property, and fault rows to
be evidenced on every platform/filesystem that is actually supported, a named
decision for the old-reader and platform matrix, an accepted performance ADR
or compensating exception, and a clean traceable release candidate. A green
test on one host does not widen that scope.

The current M1 contract explicitly includes FI-03 (unknown effect after a real
tool effect), so it is not safe to call that row out of scope while claiming
external side-effect execution. The architecture documents also state that
Pong does not guarantee observability or reversibility of an uncooperative
external host. This creates a legitimate product-scope choice, but it requires
an owner decision that narrows the claim; it is not an automatic PASS.

### FI-03 Finding

1. **What it tests:** a tool has already taken effect, the process terminates
   before the outcome is appended, and recovery must keep the operation
   `unknown` until provider/workspace evidence reconciles it.
2. **Real external effect required:** yes. The FI matrix names the point
   "after tool execution" and `I-IDEMP-04` requires provider evidence for a
   terminal outcome.
3. **Independent hook exists:** no. The current repository has no independent
   external provider/tool execution hook.
4. **Can it run without a provider:** no. The current PT-10 schedule creates
   unresolved synthetic states; it does not observe a real provider effect.
5. **Is synthetic evidence sufficient:** no. It demonstrates local unknown
   preservation only and cannot prove provider reconciliation.
6. **Nature of the gap:** both a technical evidence gap and a product-scope
   boundary. The architecture explicitly avoids claiming impossible host
   observability, while the current M1 Fault Matrix still requires the case.
7. **Scope disposition:** `FI-03 = DEFERRED` is defensible only if the owner
   formally narrows M1 to the durable local core and excludes external
   side-effect execution. Otherwise it remains a true `MUST PASS` blocker.

### Linux FI-13 Finding

1. **Semantics:** permission revocation or read corruption must produce a
   stable `PERMISSION_DENIED`, `RECOVERY_REQUIRED`, or quarantine result and
   must not publish unsafe state.
2. **Existing Windows test:** it is Windows-specific: `icacls`, an NTFS deny
   ACE, raw Windows error 5, ACL restoration, and cold reopen.
3. **Linux requirement:** an equivalent Linux row is required only if Linux is
   accepted in the M1 supported matrix.
4. **Contract reading:** the Gate is platform/filesystem-neutral and requires
   rows on every supported local platform/filesystem, but the compatibility
   matrix currently accepts none.
5. **Correct Linux semantics if accepted:** POSIX mode/permission revocation
   and read failure (and ACL/ownership only if the owner declares those as
   supported semantics), executed in a disposable environment where the test
   identity cannot bypass the denial. The repository has no such Linux test
   entry today.
6. **Scope disposition:** Linux FI-13 may be recorded `NOT_APPLICABLE` only
   after the owner excludes Linux from M1 support. If Linux remains supported,
   the row is `BLOCKED` until its evidence exists.

## Can Be Waived

Only bounded exceptions with an owner, date, exact commit, affected scope,
compensating control, and residual risk are candidates:

- v0.1 old-reader compatibility, by explicitly excluding legacy repositories;
- named platform/filesystem rows for FI-07, FI-13, or FI-14 that are not
  supported by the release;
- FI-03 only by explicitly excluding external side-effect execution from the
  M1 product claim; it cannot be waived while that claim remains; and
- missing performance rows/metrics through a bounded ADR-0015 capacity scope.

An exception does not rewrite the underlying evidence as `PASS`.

## Out of Scope

The following are already outside the settled M1 core boundary unless a later
ADR changes it: distributed consensus, automatic compensation/rollback of
external effects, provider-specific browser/database proxies, and a public
CLI/SDK/server/framework adapter. These are not new M1 Gate conditions.

Linux FI-13 is **not** automatically out of scope. The current contract is
platform-neutral, while the existing test entry is Windows-specific (`icacls`
and NTFS ACL semantics). If Linux remains an accepted M1 platform, a POSIX
permission/revocation schedule is the appropriate evidence. If Linux is
excluded from the supported M1 matrix, Linux FI-13 may be recorded as
`NOT_APPLICABLE` by the Release Owner.

## Deferred to M2+

No M2 implementation work is authorized by this document. The following remain
deferred until M1 is accepted: public Core API stabilization, CLI/SDK work,
runtime/provider adapters, remote replication, and any broader snapshot or
capacity contract not already covered by the current M1 evidence.

## Owner Action Required

The owner must freeze the supported platform/filesystem scope, decide the
external-effect claim, decide the old-reader disposition, accept or reject
ADR-0015 and ADR-0016, freeze a clean candidate, and complete the sign-off
record. Codex cannot perform those governance actions.

## Decision Table

| Item | Current Status | M1 Must Pass? | Technical Work | Owner Decision | Recommended Disposition |
|---|---|---|---|---|---|
| FI-03 external tool effect | `BLOCKED` | **YES** if M1 claims external side-effect execution | Provide an independent provider/tool harness, terminate after effect before outcome, retain provider evidence and cold-reopen reconciliation; no current hook exists | Keep the claim and supply external evidence, or explicitly exclude external side-effect execution | `MUST` for current claim; otherwise `DEFERRED` by formal scope decision |
| Linux FI-13 | `BLOCKED` for a Linux-supported matrix; `NOT_APPLICABLE` only if Linux is excluded | Conditional on Linux acceptance | If Linux is accepted, run POSIX permission/revocation/read-corruption evidence on a disposable non-root-capable environment; current entry is Windows-only | Accept Linux row and require evidence, or exclude Linux from M1 support | Do not add a Windows ACL-equivalent test without owner scope decision |
| Property testing | `NOT_PROVEN` | **YES** for each PT row in the declared CI corpus; platform qualification still needs owner scope | Bind existing Windows/Linux records and hosted-runner suite evidence to accepted rows; run further corpus only if owner chooses per-row policy | Freeze accepted rows and choose one-CI-corpus vs per-row normative policy | Keep `NOT_PROVEN` until scope and PT-13/ADR-0016 acceptance are recorded |
| Old Reader | `BLOCKED` | Required by current compatibility/release matrix unless explicitly excluded | Obtain independent v0.1 binary, fixture, hash, read/mutation/cold-reopen probes | Supply artifact or sign a bounded compatibility exclusion | `CAN_BE_WAIVED` only by explicit legacy-support exclusion |
| ADR-0015 | `OWNER_ACTION_REQUIRED` | **YES** as accepted ADR or bounded exception | Complete measurements for rows that remain in scope; do not change thresholds to fit data | Accept proposed budget or record compensating bound | `OWNER_ACTION_REQUIRED`; no automatic acceptance |
| ADR-0016 | `OWNER_ACTION_REQUIRED` | **YES** while projections remain in M1 | Review retained fixtures and PT-13/FI-10 evidence against the proposed contract | Accept or reject ADR-0016 | `OWNER_ACTION_REQUIRED`; not waiverable without removing projections from M1 |
| Traceability / candidate | `BLOCKED` | **YES** for a release-candidate claim | Commit final evidence snapshot, verify clean tree, regenerate hashes/traceability, rerun consistency | Repository owner controls candidate commit/tag | Cannot be waived while making a reproducible release claim |
| Release Owner sign-off | `OWNER_ACTION_REQUIRED` | **YES** | None technical; complete the human record | Name owner, date, scope, evidence, exceptions, risks, decision, signature | Cannot be waived |

## Explicit Disposition Labels

These labels describe what the existing contract permits; none is an owner
acceptance:

```text
FI-03       = MUST for the current external-effect claim; DEFERRED only by formal scope exclusion
Linux FI-13 = CONDITIONAL; NOT_APPLICABLE only when Linux is excluded from M1 support
Property    = NOT_PROVEN until the owner freezes the corpus/platform policy
Old Reader  = CAN_BE_WAIVED conditionally by excluding v0.1 compatibility; otherwise MUST
ADR-0015    = OWNER_ACTION_REQUIRED
ADR-0016    = OWNER_ACTION_REQUIRED while projections remain in M1
Traceability= MUST for a release-candidate claim
Sign-off    = MUST; the governance control itself is not waiverable
```

## Stale Reference Check

The active traceability record and release matrix point to commit `6cb62fb` and
successful workflow `33145714975`; no active release-matrix reference was
found pointing to an older candidate. The top-level `SHA256SUMS` currently
verifies all 451 listed files. `artifact-references.json` is a selected source
reference index rather than a one-to-one mirror of every checksum entry; a
direct packaging audit found 445 selected records, six retained Linux VM
`*-exact` files not represented in that index, and a self-entry whose recorded
hash/size is stale (a self-hash is inherently circular). This is a packaging
hygiene caveat, not a new M1 Gate condition. Before a candidate is frozen, the
release process should regenerate or explicitly label the selected index and
exclude/handle its self-entry. The retained
`compatibility/old-reader-audit.json` is different: it records historical
absence checks at `359abc306b554d592b532ebc182e543f97489043` and earlier audit
snapshots. That record is valid as historical absence evidence, but its commit
metadata is stale relative to the current candidate. Before a final candidate,
the owner/release process must either rebind it or label it explicitly as a
historical audit artifact. The bundled `build-metadata.json` likewise reports
the older `359abc` snapshot and `working_tree=CLEAN`; it is historical metadata,
not the active `6cb62fb` traceability record.

## Release-Candidate Readiness Chain

| Chain step | Current fact | Readiness |
| --- | --- | --- |
| Source commit | `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` on `dev` | Identified |
| Local evidence | Windows stable/MSRV and focused evidence retained | Present |
| Linux VM evidence | Debian 13, ext4, Rust 1.85 VM records retained | Present; not MSRV |
| GitHub Linux evidence | Run `33145714975`, ext4, all 13 commands exit `0` | Present; owner acceptance pending |
| GitHub macOS evidence | Run `33145714975`, filesystem `unknown`, all 13 commands exit `0` | Present; do not call APFS/physical Mac |
| Artifact hashes | Top-level `SHA256SUMS` verifies its 451 entries; the selected `artifact-references.json` index has a stale self-entry and six unindexed Linux VM `*-exact` files | Packaging caveat to reconcile or label before candidate freeze; not a new Gate condition |
| Final test | `artifact_consistency` passes its current assertions | Verified for the existing contract; it intentionally does not cryptographically self-hash the selected index |
| Clean commit | Working tree contains uncommitted evidence/documentation changes | Missing |
| Candidate tag | No release tag | Missing; do not create in this audit |
| Owner sign-off | `UNASSIGNED` / `PENDING` | Missing |

This chain is evidence readiness, not a release candidate. The first seven
steps do not compensate for the missing clean commit, tag, or human decision.

## What the Linux VM Closed This Round

This round did not rerun the already successful Linux corpus or native suite.
The retained Linux VM evidence closes only these **evidence rows**:

- Linux VM Rust 1.85 `fmt`, `check`, full `test`, and clippy: `PASS` for that
  VM and toolchain, not MSRV 1.78 evidence;
- Linux-native property records for the retained PT corpus, including the
  10,000-case records and PT-14's 10,008 executed cases;
- Linux `tmpfs` FI-14 CAS, metadata, and journal exhaustion: `PASS` for the
  exercised rows, with errno `28` and Pong `RESOURCE_EXHAUSTED`;
- Linux `tmpfs` FI-07-related CAS resource behavior: `PASS` for the exercised
  row; and
- top-level `SHA256SUMS` and raw-log hash verification. The selected
  `artifact-references.json` index needs regeneration or an explicit historical/
  selected-index label before a candidate is frozen; this does not alter the
  Gate's checksum rule.

It did **not** close FI-03, Linux FI-13, the overall property acceptance,
Old Reader, ADR-0015, ADR-0016, traceability, or Release Owner sign-off.

## Final Scope Conclusion

No owner scope decision has yet been recorded, so no blocker is removed by this
document. The current authoritative result remains:

```text
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```

The immediate next step is owner scope/acceptance review, not more M1 feature
development and not another run of the already retained Linux evidence.
