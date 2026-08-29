# M1 Scope Freeze

**Prepared:** 2026-08-29  
**Candidate commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Branch:** `dev`  
**Native evidence workflow:** `33145714975`  
**Working tree:** `DIRTY`  
**Release tag:** none  
**Decision authority:** Release Owner  
**Current Gate:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`

**Owner Decision Draft:** `APPROVED_FOR_FINALIZATION`  
**Formal Owner acceptance:** `PENDING`

> **Final acceptance overlay (2026-08-29):** Release Owner `NJHTR` formally
> accepted this bounded scope by textual declaration. The draft/pending labels
> below are retained proposal history. Current state is
> `OWNER_DECISION = ACCEPTED`, `M1_SCOPE_FREEZE = FROZEN` at source commit
> `58e9e875...`, `FINAL_CI = PENDING`, and
> `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`.

This is a proposed scope freeze for Release Owner review. It bounds the M1
claim without promoting incomplete evidence, changing a Gate invariant,
accepting ADR-0015 or ADR-0016, creating a tag, or signing a waiver. Until the
Owner records the effective scope and decisions in the sign-off record, the
labels below remain proposals.

## Supported Platforms

The proposed M1 support/qualification scope is:

1. **Windows x86_64 on NTFS** as a local supported platform.
2. **Linux x86_64 on ext4** as a local supported platform, with Debian 13
   Linux-native VM evidence retained separately from Docker evidence.
3. **macOS GitHub-hosted runner** as CI qualification evidence only; the
   retained filesystem is `unknown`. This is not a physical-Mac or APFS claim.

The Owner must either accept these rows or remove named rows from the release
scope. Docker overlay, Docker-backed ext4 volumes, and the dedicated Linux
`tmpfs` mount are evidence environments, not additional support claims.

## Supported Filesystems

The proposed filesystem scope is:

| Platform | Filesystem claim | Boundary |
| --- | --- | --- |
| Windows x86_64 | NTFS | Native local support; Windows ACL evidence is applicable |
| Linux x86_64 | ext4 | Native local support; Linux VM evidence is Rust 1.85, not MSRV 1.78 |
| macOS GitHub-hosted runner | `unknown` | Qualification only; no APFS or physical-host claim |

Resource-fault evidence on Linux `tmpfs` is retained as a disposable resource
qualification. It does not prove generic ext4 or Windows quota behavior.

## Required Faults

For every local platform/filesystem row that the Owner accepts, FI-01, FI-02,
FI-04 through FI-12, and FI-13 permission/security semantics are required by
the existing M1 contract. FI-07 and FI-14 are required for each accepted
resource model. A retained test on one row does not generalize to another row.

| Fault | Proposed M1 classification | Platform/filesystem scope | Evidence boundary |
| --- | --- | --- | --- |
| FI-01 | `REQUIRED` | Every accepted local row | Intent pre-durability boundary and cold-reopen result |
| FI-02 | `REQUIRED` | Every accepted local row | Intent durable before execution; no false success |
| FI-03 | `DEFERRED` (conditional) | External provider/tool side effects excluded from the M1 claim | Synthetic unknown preservation only today; real `EXTERNAL` evidence is absent |
| FI-04 | `REQUIRED` | Every accepted local row | Outcome/WAL tail recovery and quarantine/unknown result |
| FI-05 | `REQUIRED` | Every accepted local row | Durable outcome to publication/ref recovery |
| FI-06 | `REQUIRED` | Every accepted local row | Metadata transaction pre-state/post-state boundary |
| FI-07 | `REQUIRED` | Each accepted local resource/filesystem model | Linux `tmpfs` `NATIVE` row retained; other models need their own evidence |
| FI-08 | `REQUIRED` | Every accepted local row | CAS publication and directory durability boundary |
| FI-09 | `REQUIRED` | Every accepted local row | Ref CAS and retry idempotency |
| FI-10 | `REQUIRED` | Every accepted projection row | Projection rebuild, generation binding, and FI-10 crash schedule |
| FI-11 | `REQUIRED` | Every accepted local row | Redaction and persisted-byte security boundary |
| FI-12 | `REQUIRED` | Every accepted generation/migration row | Old-or-fully-verified-new generation visibility |
| FI-13 | `REQUIRED` | Windows NTFS and Linux ext4 if those rows remain supported | Windows `NATIVE` ACL row retained; Linux POSIX permission row is missing |
| FI-14 | `REQUIRED` | Each accepted local resource/filesystem model | Linux `tmpfs` CAS/metadata/journal rows retained; other models need evidence |

`OPTIONAL` has no current FI-01..FI-14 assignment. `WAIVED` is not assigned
by Codex. The Owner may convert a named row to a bounded exception only by
explicitly excluding the affected capability or platform and recording the
residual risk. Such an exception does not turn the row into `PASS`.

## Deferred Faults

### FI-03: external side-effect reconciliation

The current Pong Core has no independent external provider/tool execution
interface. PT-10 synthetic schedules prove only that an unresolved local intent
does not become success. They do not prove reconciliation after a real tool
effect. Therefore:

```text
FI-03 = DEFERRED TO M2+
```

This disposition is valid only if the Owner explicitly narrows the M1 claim to
the durable local core and excludes external side-effect execution. If M1
claims safe execution of external effects, FI-03 remains `REQUIRED` and
`BLOCKED`; a fake provider must not be added to close the row.

## Not Applicable Faults

No local Windows or Linux FI row is automatically not applicable. Under the
proposed support scope, FI-13 remains required for both Windows NTFS and Linux
ext4 because the normative Gate requires permission/read-failure behavior on
supported local rows. The macOS GitHub-hosted row is qualification-only and
does not create a separate local permission/resource support claim.

If the Owner removes Linux from M1 support, the Linux FI-13 row may then be
recorded as `NOT_APPLICABLE`; until that decision is recorded, Linux FI-13 is
`BLOCKED`, not N/A. No FI row is currently marked `WAIVED`.

## Property Policy

The proposed policy is:

- **Canonical normative corpus:** deterministic repository-defined corpus,
  10,000 executed cases per generated property in CI, with seed, generator,
  invariant, requested/executed count, toolchain, platform/filesystem, exit
  code, raw log, and shrink trace on failure.
- **Generated properties:** PT-01..PT-12 and PT-14. PT-14 may report 10,008
  cases because its harness expands nine migration failpoints.
- **Fixed contract suite:** PT-13 is an event/projection fixture and crash
  contract, not a generated 10,000-case property; acceptance is coupled to
  ADR-0016.
- **Cross-platform policy:** one canonical normative corpus plus qualification
  and smoke checks on accepted rows. The existing contract does not require
  automatic multiplication of 10,000 cases by platform.
- **No inferred results:** no case count is claimed for hosted macOS merely
  because its standard suite exited zero.

Evidence classes are distinct:

| Class | Meaning | Release use |
| --- | --- | --- |
| `NORMATIVE` | Retained canonical generated corpus with complete metadata | Satisfies the canonical corpus requirement for its declared scope |
| `QUALIFICATION` | Platform-specific selected/focused property evidence | Supports an accepted platform row; not a replacement for missing corpus metadata |
| `SMOKE` | Fast sanity or standard suite without normative retention | Cannot satisfy the normative corpus requirement |
| `SUPPLEMENTARY` | Diagnostic, Docker, or toolchain-variant evidence | Risk/context only |

Current Windows and Linux VM property records are retained. Linux VM records
run Rust 1.85, not MSRV 1.78. Hosted Linux/macOS Run `33145714975` records
quality/focused suites but not a complete per-property normative corpus for
hosted macOS. Overall property status remains `NOT_PROVEN` pending Owner
scope acceptance and PT-13/ADR-0016 acceptance.

## Old Reader Policy

No pre-M1 Pong release exists in the checked history or retained artifacts, and
no independent v0.1 reader was found. The proposed first-release policy is:

```text
Old Reader = WAIVED (bounded compatibility-scope exception, pending Owner acceptance)
```

The exception must state that M1 provides no backward-compatibility guarantee
for unreleased/internal pre-M1 repository states. It must not be represented
as an old-reader test pass, and it must name the exact commit, owner, date,
scope, and residual risk. Without that signed decision, Old Reader remains
`BLOCKED`.

## Performance Policy

M1 is proposed as a durability/correctness/recovery baseline with measured
performance, not an unlimited capacity certification. The retained
`m1-perf-0.3` package is measurement evidence for recorded workloads. ADR-0015
remains `Proposed` and must not be edited or accepted automatically.

The Owner must choose one of:

1. accept ADR-0015 for an explicitly named platform/filesystem and workload
   scope after required runs are present; or
2. record a bounded baseline/capacity exception with excluded rows, operating
   limits, and residual risk.

No new threshold is created by this scope freeze. Missing native-row runs and
unmeasured memory/CPU behavior remain acceptance gaps until the Owner decides
their scope.

## Projection Policy

Projection implementation and executable evidence are `PASS` for the recorded
current-host and focused-run scope: event envelopes, cursor/ledger identity,
generation binding, rebuild, idempotency, unknown-event handling, and FI-10
failpoints are retained. ADR-0016 is still `Proposed`.

While projections remain in M1, the Owner must accept or reject ADR-0016
against the retained fixtures and invariants. The implementation result does
not itself constitute ADR acceptance, so the release status remains
`OWNER_ACTION_REQUIRED`.

## Release Acceptance Requirements

An M1 release claim requires all of the following, after the Owner freezes the
scope:

1. Required FI rows are evidenced for every accepted local platform,
   filesystem, and resource model; any excluded row has a signed bounded
   exception.
2. The canonical property corpus and required qualification records are
   retained with exact commit, platform/filesystem, seed, case count, and raw
   output metadata.
3. Old Reader disposition is recorded as a real independent-reader result or
   a bounded compatibility exception.
4. ADR-0015 is accepted or has a compensating bound; ADR-0016 is accepted if
   projections remain in M1.
5. Evidence is frozen against one clean candidate commit. The active records
   currently identify commit `6cb62fb455e92ab731a4bb5233856d10c1f1ce93` and
   workflow `33145714975`; the working tree is still dirty and no tag exists.
6. Top-level `SHA256SUMS` and artifact references are regenerated and verified
   for that exact candidate. The current audit found 451 checksum entries,
   445 selected reference entries, six retained Linux VM `*-exact` files
   outside the selected index, and a stale self-entry; this is packaging
   hygiene to reconcile or label before freeze, not a new Gate condition.
7. A named Release Owner completes `M1_RELEASE_OWNER_SIGNOFF.md` with scope,
   evidence, exceptions, risks, decision, date, and signature.

## Scope Freeze Decision Table

| Item | Status | M1 Required? | Evidence | Owner Decision | Final Disposition |
|---|---|---|---|---|---|
| FI-03 | `BLOCKED` | Yes while external effects are claimed; otherwise no | PT-10 synthetic unknown-preservation schedules; no provider hook | Exclude external side effects from M1 or supply real external evidence | Proposed `DEFERRED TO M2+`; pending scope decision |
| FI-07 | `BLOCKED` | Yes for each accepted resource/filesystem model | Linux `tmpfs` native row; other accepted models incomplete | Accept only named resource models or exclude missing rows | `REQUIRED`; pending scope |
| FI-13 | `BLOCKED` | Yes for accepted local Windows/Linux rows | Windows NTFS ACL row; Linux POSIX row absent | Keep Linux supported and add evidence, or remove Linux from M1 | `REQUIRED` for retained local rows; Linux N/A only after exclusion |
| FI-14 | `BLOCKED` | Yes for each accepted resource/filesystem model | Linux `tmpfs` CAS/metadata/journal rows; other models incomplete | Accept named resource models or exclude missing rows | `REQUIRED`; pending scope |
| Property | `NOT_PROVEN` | Yes for canonical corpus and accepted qualification rows | Windows/Linux VM retained records; hosted runner focused evidence | Freeze one-corpus vs per-row policy and accepted platforms | Proposed one canonical 10,000-case corpus plus qualification |
| Old Reader | `BLOCKED` | Required unless legacy compatibility is excluded | Absence audit; no independent v0.1 reader | Supply real reader/fixture or sign bounded compatibility exception | Proposed `WAIVED`; pending Owner signature |
| ADR-0015 | `OWNER_ACTION_REQUIRED` | Yes as accepted ADR or bounded exception | `m1-perf-0.3` measurement package | Accept thresholds for named scope or record compensating bound | Baseline/measurement policy proposed; ADR remains Proposed |
| ADR-0016 | `OWNER_ACTION_REQUIRED` | Yes while projections remain in M1 | PT-13/FI-10 implementation and focused evidence | Accept or reject the projection contract | Keep projections in M1; acceptance pending |
| Traceability | `BLOCKED` | Yes for a release-candidate claim | HEAD/workflow/artifact references and checksum verification | Freeze clean candidate and reconcile/label selected index | No release candidate until clean freeze |
| Release Owner | `OWNER_ACTION_REQUIRED` | Yes | Unsigned pending sign-off template | Name owner, scope, exceptions, risks, decision, signature | Human acceptance required |

## Final Status

This scope freeze does not remove any blocker. It narrows the decisions that
the Release Owner must make and prevents automatic expansion of M1 into every
platform/filesystem/resource combination.

```text
M1_SCOPE_FREEZE = PROPOSED / OWNER_ACTION_REQUIRED
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```

## Owner Decision Draft Overlay

The supplied Owner Decision Draft approves the Windows x86_64/NTFS and Linux
x86_64/ext4 support rows, macOS GitHub-hosted qualification only, the bounded
Linux `tmpfs` resource model, and the one-canonical-corpus property policy for
final-candidate preparation. It does not execute a waiver, accept an ADR, or
freeze a release candidate. The formal status is therefore:

```text
M1_SCOPE_FREEZE = APPROVED_FOR_FINALIZATION / FORMAL_ACCEPTANCE_PENDING
```

The earlier tables remain the historical proposal/audit context. The current
effective dispositions are recorded in `M1_FINAL_CANDIDATE_READINESS.md`.
