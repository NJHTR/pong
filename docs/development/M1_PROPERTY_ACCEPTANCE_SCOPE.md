# M1 Property Acceptance Scope

**Prepared:** 2026-08-29  
**Candidate commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Native evidence workflow:** `33145714975`  
**Decision authority:** Release Owner  
**Current status:** `NOT_PROVEN` / owner scope acceptance pending  
**M1 Release Gate:** `BLOCKED / NOT_PASSED`

**Owner Decision Draft:** `APPROVED_FOR_FINALIZATION`  
**Formal acceptance:** `PENDING`

> **Final acceptance overlay (2026-08-29):** Owner `NJHTR` accepted one
> canonical deterministic 10,000-case corpus per generated property plus
> accepted-platform qualification; no 10,000-by-every-platform multiplication
> is required. The pending statements below are historical proposal context.

This document freezes a proposed property-evidence policy. It does not add a
new Gate condition, weaken an existing invariant, accept a platform row, or
turn a smoke run into normative evidence.

## Canonical Normative Corpus

The canonical normative corpus is the repository-defined deterministic
property corpus with **10,000 executed cases per generated property** in CI.
The corpus retains the property ID, generator/invariant identity, seed,
requested and executed case count, toolchain, platform/filesystem, exit code,
raw log, and a shrink/failure trace when relevant.

The generated properties are PT-01 through PT-12 and PT-14. PT-14 records
10,008 executions when its nine migration failpoints are expanded by the
harness. PT-13 is a fixed event/projection fixture and crash-contract suite,
not a generated 10,000-case property; its release acceptance remains coupled
to ADR-0016.

The current contract requires a deterministic 10,000-case CI corpus, but it
does **not** state that the same corpus must be independently multiplied by
every platform. A stricter per-platform 10,000-case policy would be a separate
Release Owner decision. No case count is inferred for a platform where it was
not retained.

## Proposed Accepted Platforms

These rows are proposed for Owner review; none is formally accepted yet:

| Platform | Filesystem identity | Evidence boundary |
| --- | --- | --- |
| Windows x86_64 | NTFS | Current-host stable/MSRV gates and retained normative property records |
| Linux x86_64 | ext4 | Debian 13 Linux-native VM records on Rust 1.85; GitHub-hosted Linux evidence on the declared runner |
| macOS GitHub-hosted runner | `unknown` | Run `33145714975`; this is not a physical Mac or APFS claim |

Docker overlay, Docker-backed ext4 volumes, and the disposable Linux `tmpfs`
mount remain supplemental evidence environments. They are not added to the
supported platform/filesystem scope by this policy.

## Evidence Classes

| Class | Meaning in this policy | May satisfy canonical corpus? |
| --- | --- | --- |
| `NORMATIVE` | Retained deterministic 10,000-case generated corpus with complete metadata | Yes, for the declared CI corpus |
| `QUALIFICATION` | Platform-specific focused or selected property evidence used to qualify an accepted row | Yes, for row qualification; not a replacement for missing canonical metadata |
| `SMOKE` | Fast sanity checks or standard suite execution without the normative corpus record | No |
| `SUPPLEMENTARY` | Diagnostic, Docker, Linux VM toolchain-variant, or other supporting evidence outside the canonical claim | No; may inform risk and owner scope |

The evidence class in this document is a property-policy class. Machine
artifact records retain their original environment labels (for example
`LINUX_NATIVE`) and are not rewritten by this scope decision.

## Platform Qualification Policy

For every platform/filesystem row the Owner accepts, retain:

1. `cargo fmt`, `cargo check`, complete test, and clippy results with the
   declared stable/MSRV toolchains where available;
2. the common recovery, migration, projection, and compatibility checks;
3. selected property evidence sufficient to demonstrate that the canonical
   corpus is portable to that row; and
4. platform/filesystem, repository commit, command, exit code, and artifact
   references.

The retained Windows and Linux VM records satisfy the normative corpus shape
for the properties they cover. The Linux VM used Rust 1.85 because Rust 1.78
was unavailable there; this is not MSRV proof. Run `33145714975` provides
GitHub-hosted Linux and macOS quality/focused evidence, but no complete
retained per-property normative corpus record for hosted macOS.

## Required Cross-Platform Checks

The following checks remain required on each accepted row, independent of the
case-count policy:

- canonical IDs and digests are stable;
- CAS, metadata, event, idempotency, recovery, redaction, and migration
  invariants do not silently change status;
- PT-09/PT-10/PT-14 retain cold-reopen and generation/unknown-state checks;
- PT-13 retains projection fixtures and source-event immutability evidence;
- no mixed-generation state is exposed; and
- raw logs and machine-readable records are bound to the candidate commit.

These checks do not authorize a new test matrix or require 10,000 multiplied
by every platform unless the Owner explicitly chooses that policy.

## Current Evidence Register

| Platform | Corpus record | Current result | Classification | Remaining decision |
| --- | --- | --- | --- | --- |
| Windows x86_64 / NTFS | `windows-property-corpus-10000-20260828.json`; PT-09/PT-10/PT-14 retained separately | 10,000/property where recorded; PT-14 10,008; exit `0` | `NORMATIVE` for recorded rows | Owner acceptance of row and one-corpus policy |
| Linux VM `rtc-node-1` / ext4 | `linux-native-vm-*-10000-20260828.json` plus raw logs | PT-01..08/11/12, PT-09, PT-10: 10,000; PT-14: 10,008; exit `0` | `NORMATIVE` for Rust 1.85 VM rows; `SUPPLEMENTARY` for MSRV claim | Owner acceptance of row and toolchain scope |
| Linux GitHub-hosted runner / ext4 | Run `33145714975` platform artifact | Standard suites exit `0`; no complete per-property corpus record retained | `QUALIFICATION` / `SMOKE` | Owner decides whether hosted Linux is in scope and whether more corpus retention is required |
| macOS GitHub-hosted runner / filesystem `unknown` | Run `33145714975` platform artifact | Standard suites exit `0`; no complete per-property corpus record retained | `QUALIFICATION` / `SMOKE` | Owner decides whether this unknown-filesystem row is supported |

The overall property status remains `NOT_PROVEN` because no Owner has frozen
the accepted rows or this one-corpus qualification policy, and PT-13 remains
pending ADR-0016 acceptance.

## Required Retained Evidence

The final candidate package must retain, for every property record used in the
release decision:

- candidate commit and branch identity;
- platform and filesystem identity (with `unknown` preserved where observed);
- corpus/property ID, invariant, seed, requested and executed counts;
- command, toolchain, lockfile identity, exit code, and elapsed time;
- raw output and failure/shrink trace when applicable; and
- artifact path and SHA-256 reference.

No physical macOS evidence is claimed. No hosted-runner filesystem is renamed
to APFS without an observed filesystem record.

## Owner Decision Required

The Release Owner must record, in `M1_RELEASE_OWNER_SIGNOFF.md`,

1. the accepted platform/filesystem rows;
2. whether one canonical 10,000-case CI corpus plus per-row qualification is
   sufficient, or whether 10,000 cases are required on every accepted row;
3. the disposition of missing hosted-macOS per-property records; and
4. the residual risk and any bounded exception.

Until those decisions are recorded, this scope is a proposal and
`PROPERTY_TESTING = NOT_PROVEN`.

## Owner Decision Draft Overlay

The supplied Owner Decision Draft selects one canonical deterministic corpus of
10,000 cases per generated property plus cross-platform qualification. It does
not require 10,000 cases multiplied by every platform. Under that bounded
policy, the retained Windows and Linux corpus records and hosted-macOS
qualification are technically sufficient for finalization:

```text
PROPERTY_TESTING = PASS (BOUNDED TECHNICAL EVIDENCE)
FORMAL_POLICY_ACCEPTANCE = PENDING
```

The original status above remains the historical pre-decision audit context;
no missing hosted-macOS case count is inferred and no smoke run is promoted to
normative evidence.
