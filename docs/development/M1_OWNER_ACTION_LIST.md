# M1 Release Owner Action List

**Prepared:** 2026-08-28  
**Candidate evidence commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Native evidence workflow:** `33145714975`  
**Current status:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`

This is a human decision checklist. Codex must not fill, sign, or infer any of
the decisions below.

| # | Decision | Risk | Evidence to review | Required signature / record |
| ---: | --- | --- | --- | --- |
| 1 | Confirm required M1 platform scope: Windows x86_64; Linux x86_64/ext4; GitHub-hosted macOS runner (filesystem `unknown`) | Scope may overstate an unqualified filesystem; hosted macOS is not a physical-Mac claim | `M1_RELEASE_SCOPE_DECISION.md`, `M1_COMPATIBILITY_MATRIX.md`, Run `33145714975` artifacts | Owner name, scope, date, and exact commit in `M1_RELEASE_OWNER_SIGNOFF.md` |
| 2 | Accept or reject the FI-03 deferral; M1 must not claim external provider side-effect safety if accepted | Real tool effects after process termination are unverified; blind retry/reconciliation risk remains | `M1_FAULT_SCOPE_DECISION.md`, `artifacts/m1-fault-matrix.json`, synthetic PT-10 records | Explicit FI-03 disposition and residual risk in sign-off |
| 3 | Accept Linux FI-13 as `NOT_APPLICABLE FOR M1`; defer POSIX/ACL qualification | Linux permission/revocation behavior remains untested | `M1_FAULT_SCOPE_DECISION.md`, Windows NTFS FI-13 evidence | Owner initials/date and accepted Linux FI-13 scope in sign-off |
| 4 | Decide Old Reader disposition: waive/exclude pre-M1 unreleased compatibility, or supply a real historical reader | Legacy/internal repositories may be unreadable; no backward guarantee exists | `compatibility/old-reader-audit.json` / `.log`, compatibility matrix | Signed compatibility exception or attached independent reader artifact and probe |
| 5 | Confirm Property policy: one primary normative corpus of 10,000 cases/property plus cross-platform smoke/selected evidence | A platform-specific divergence could remain undiscovered; policy differs from per-row 10,000 | `M1_PROPERTY_SCOPE_DECISION.md`, Windows/Linux property records, hosted macOS logs | Owner records corpus policy, accepted rows, and date |
| 6 | Decide ADR-0015: accept a bounded measurement-only baseline or adopt explicit release thresholds and bounds | An informational baseline alone does not satisfy the Gate; it provides no latency/capacity guarantee outside the named scope | `M1_PERFORMANCE_SCOPE_DECISION.md`, `M1_PERFORMANCE_ACCEPTANCE.md`, ADR-0015, `m1-perf-0.3` | ADR-0015 decision or bounded exception with compensating controls and explicit unmeasured rows |
| 7 | Decide ADR-0016 acceptance for the generation-bound projection contract | Rebuild, cursor, migration, and unknown-event semantics remain governance-unaccepted | `M1_PROJECTION_ACCEPTANCE.md`, ADR-0016, PT-13/FI-10 evidence | Accept/reject ADR-0016 with owner/date/scope; do not edit it automatically |
| 8 | Confirm the fault scope: FI-01/02/04/05/06/08/09/10/11/12 required; FI-07/FI-14 where applicable; Windows FI-13 only; FI-03 deferred | Excluded resource/provider rows may hide platform-specific failures | `M1_FAULT_SCOPE_DECISION.md`, fault matrix, retained native evidence | Fault-scope decision and residual-risk statement in sign-off |
| 9 | Approve the release-candidate transition only after the final evidence snapshot is committed, clean, traceable, and rechecked | Dirty tree, stale bundle references, or no tag cannot provide reproducible release provenance | `traceability/release-traceability.json`, `artifact-references.json`, `SHA256SUMS`, artifact-consistency result | Repository owner regenerates and parity-checks the references/checksums, creates/approves the controlled candidate, and Codex creates no tag here |
| 10 | Complete and sign `M1_RELEASE_OWNER_SIGNOFF.md` with `ACCEPTED` or `REJECTED` | Missing signature removes the governance control itself | All documents and retained artifacts above | Named human owner signature, date, exact commit, scope, exceptions, risks, decision |

## Decision Order

The owner should make decisions 1 through 8 first, then perform the clean
candidate/provenance step 9, and finally complete step 10. A `REJECTED` or
`PENDING` decision keeps M1 blocked.

## Current Technical Boundary

The current repository already contains the requested implementation evidence
for Windows, Linux-native VM, and the GitHub-hosted macOS runner within their
declared limits. This list separates those technical facts from the decisions
that only the Release Owner can make. It does not authorize M2 or create a
release tag.
