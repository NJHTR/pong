# M1 Fault Matrix

**Prepared:** 2026-08-29  
**Candidate commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Branch:** `dev`  
**Decision authority:** Release Owner  
**Current decision:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`

This document closes the Fault Matrix scope analysis for FI-03, FI-07, and
FI-14. It records the evidence boundary without widening M1, promoting
synthetic evidence to native evidence, accepting a waiver, or changing the
M1 Gate definition.

## Scope

### Supported Platforms

The proposed M1 scope in `M1_SCOPE_FREEZE.md` is:

| Platform | M1 role | Filesystem claim |
| --- | --- | --- |
| Windows x86_64 | Local supported platform | NTFS |
| Linux x86_64 | Local supported platform | ext4 |
| macOS GitHub-hosted runner | CI qualification only | `unknown`; not APFS or a physical-Mac claim |

### Supported Filesystems

| Platform | Supported/qualified filesystem | Boundary |
| --- | --- | --- |
| Windows x86_64 | NTFS | Proposed local support row |
| Linux x86_64 | ext4 | Proposed local support row |
| macOS GitHub-hosted runner | `unknown` | Qualification only; no local filesystem support claim |

The dedicated Linux 64 MiB `tmpfs` is a disposable resource qualification
environment, not an additional supported filesystem claim. Docker overlay and
Docker-backed ext4 records are supplemental/external evidence and do not
establish native support rows.

## Required Faults

For every local platform/filesystem row accepted by the Owner, the existing M1
contract requires `FI-01`, `FI-02`, `FI-04` through `FI-13`. `FI-07` and `FI-14`
are required for each resource/filesystem model that the Owner accepts. `FI-03`
is conditional: it is required if M1 claims safe external side-effect
execution, and can be deferred to M2+ only after the Owner explicitly excludes
that capability from M1.

No `OPTIONAL` fault is assigned in the current M1 matrix. No row is currently
`WAIVED` or formally accepted; the scope freeze remains a proposal pending
Owner sign-off.

## FI-03 Contract And Disposition

### Exact requirement

The normative fault contract is:

```text
after real external tool/provider execution
before outcome append
-> operation remains unknown until provider/workspace evidence reconciles it
-> blind retry is blocked
```

This is not equivalent to a local synthetic crash schedule. It requires a real
external effect, an abrupt process termination at the boundary, a restart, and
provider/workspace reconciliation evidence.

### Current implementation/evidence

The current Pong Core has no independent external provider execution hook,
provider interface, side-effect adapter, or subprocess/provider contract. The
retained PT-10 schedules in `tests/property_recovery_migration.rs` prove only
that an unresolved local intent is not promoted to `succeeded`; they are
`SYNTHETIC` groundwork, not a real external-effect run.

### Status

`FI-03 = DEFERRED_PENDING_OWNER_DECISION`

If the Owner keeps external side-effect execution in M1, the row remains
`BLOCKED` and needs an external schedule. If the Owner explicitly narrows M1 to
the durable local core and excludes external side effects, the row may be
recorded as deferred to M2+ in the signed scope decision. Codex does not make
that decision and no fake provider is added here.

### Minimum real closure (not executed)

1. Define the smallest provider/tool execution interface and its effect
   identity/reconciliation input.
2. Execute a real side effect in a disposable harness.
3. Terminate the process after the effect and before outcome append.
4. Cold-reopen and reconcile the effect; prove `unknown`/terminal handling and
   blocked blind retry.
5. Retain the provider identity, command, interruption point, exit code, raw
   output, recovery result, and evidence class `EXTERNAL` or `NATIVE` as
   applicable.

This is future technical work only if the Owner retains the capability in M1;
it was not implemented in this pass.

## FI-07 Required Matrix

### Exact requirement

`FI-07` injects a CAS short write or `ENOSPC` during staging. The expected result
is that the staging object is unreachable or quarantined, no partial object is
readable or referenced, and committed history remains valid after restart.

### Rows

| Platform | Filesystem/resource model | Classification | Existing evidence | Result |
| --- | --- | --- | --- | --- |
| Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs` | `QUALIFIED` resource row | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` and `artifacts/m1-release-evidence/fault/linux-native-fi14-cas_write-20260828.log`; real `errno=28`, Pong `RESOURCE_EXHAUSTED`, cleanup and cold reopen | `PASS` for this `tmpfs` row only; does not generalize to ext4 |
| Linux x86_64 local support | Root `ext4` | `REQUIRED` if Linux remains supported | No real ext4 quota/short-write row retained; synthetic CAS short/quota tests only | `BLOCKED / NOT_EXECUTED` |
| Windows x86_64 local support | `NTFS` | `REQUIRED` if Windows remains supported | Synthetic short-write/quota tests only; no safe native disk-full/quota harness executed | `BLOCKED / NOT_EXECUTED` |
| macOS GitHub-hosted qualification | `unknown` | `NOT_APPLICABLE` to a local resource claim | No filesystem identity and no local resource-support claim | Qualification row only; no APFS result inferred |

The retained Docker records are external/supplemental evidence and are not
substituted for the Linux ext4 or Windows NTFS required rows.

## FI-14 Required Matrix

### Exact requirement

`FI-14` injects disk-full/resource exhaustion during CAS object, metadata, or
journal writes. The expected result is `RESOURCE_EXHAUSTED`, no false
commit/publication, preservation of committed history, and successful cold
reopen after the resource is restored.

### Rows

| Platform | Filesystem/resource model | Classification | Existing evidence | Result |
| --- | --- | --- | --- | --- |
| Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs`, CAS write | `QUALIFIED` resource row | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` and `artifacts/m1-release-evidence/fault/linux-native-fi14-cas_write-20260828.log`; exit `0`, `errno=28`, cold reopen passed | `PASS` for this `tmpfs` row only |
| Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs`, metadata write | `QUALIFIED` resource row | `artifacts/m1-release-evidence/fault/linux-native-fi14-metadata-20260828.json` and `artifacts/m1-release-evidence/fault/linux-native-fi14-metadata_write-20260828.log`; exit `0`, `errno=28`, cold reopen passed | `PASS` for this `tmpfs` row only |
| Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs`, journal write | `QUALIFIED` resource row | `artifacts/m1-release-evidence/fault/linux-native-fi14-journal-20260828.json` and `artifacts/m1-release-evidence/fault/linux-native-fi14-journal_write-20260828.log`; exit `0`, `errno=28`, cold reopen passed | `PASS` for this `tmpfs` row only |
| Linux x86_64 local support | Root `ext4`, CAS/metadata/journal | `REQUIRED` if Linux remains supported | No real ext4 `ENOSPC` schedule retained | `BLOCKED / NOT_EXECUTED` |
| Windows x86_64 local support | `NTFS`, CAS/metadata/journal | `REQUIRED` if Windows remains supported | No safe native disk-full/quota schedule retained | `BLOCKED / NOT_EXECUTED` |
| macOS GitHub-hosted qualification | `unknown` | `NOT_APPLICABLE` to a local resource claim | Filesystem identity is unknown; no APFS claim | Qualification row only |

The Linux `tmpfs` records are real Linux-native VM evidence for that disposable
resource model. They are not generic ext4 evidence, Windows quota evidence, or
macOS evidence.

## Deferred Faults

| Fault | Disposition | Condition |
| --- | --- | --- |
| FI-03 | `DEFERRED_PENDING_OWNER_DECISION` | Valid only if the Owner excludes external side-effect execution from the M1 claim; otherwise `BLOCKED` and an external schedule is required. |

No other FI-01..FI-14 row is deferred by this document.

## Not Applicable

- FI-07 and FI-14 are `NOT_APPLICABLE` for a local macOS support claim because
  the retained macOS runner is qualification-only and its filesystem is
  `unknown`. This is not an APFS or physical-Mac disposition.
- Docker overlay, Docker-backed ext4, and the dedicated Linux `tmpfs` are not
  automatically added as supported M1 platform/filesystem rows. The tmpfs rows
  remain explicitly retained qualification evidence.
- FI-03 is not automatically `NOT_APPLICABLE`; it is conditional on the
  Owner's external-side-effect scope decision.

## Waived

`NONE ASSIGNED BY CODEX`.

A Release Owner may record a bounded exception for missing FI-07/FI-14 rows by
excluding the named platform/resource model from the M1 support claim. Such an
exception must name the exact commit, scope, residual risk, compensating
control, owner, and date. It does not change the underlying evidence row to
`PASS`. FI-03 may be excluded only by explicitly excluding external side-effect
execution; it cannot be waived while that capability is claimed.

## Property

The current `M1_PROPERTY_SCOPE_DECISION.md` proposes one deterministic,
retained 10,000-case corpus per generated property, plus platform
qualification. It does not silently require 10,000 cases multiplied by every
platform.

Existing records include Windows and Linux VM corpus artifacts bound to commit
`6cb62fb455e92ab731a4bb5233856d10c1f1ce93`. The GitHub-hosted Linux and macOS
run `33145714975` has successful standard/focused suites, but it does not
retain a complete per-property normative corpus for the hosted macOS row. No
case count is inferred from those logs, and no physical-Mac evidence is
claimed.

`PROPERTY_TESTING = NOT_PROVEN` pending Owner acceptance of the exact platform
scope and corpus policy. This fault-closure document does not rerun or expand
the property corpus.

## Final Fault Matrix

| Fault | Platform | Filesystem | Evidence | Class | Result | M1 Gate |
|---|---|---|---|---|---|---|
| FI-03 | Proposed local platforms; provider boundary | N/A | `artifacts/m1-fault-matrix.json`; PT-10 unresolved schedules | `SYNTHETIC` | No real external effect/interruption/reconciliation executed | `BLOCKED` unless Owner excludes external effects and records deferral |
| FI-07 | Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs` | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` + raw log | `NATIVE` | `PASS` for tmpfs row only | Overall matrix `BLOCKED` |
| FI-07 | Linux x86_64 local support | Root `ext4` | No retained native ext4 row | `UNAVAILABLE` | Not executed | `BLOCKED` |
| FI-07 | Windows x86_64 local support | `NTFS` | No retained native quota/disk-full row | `UNAVAILABLE` | Not executed | `BLOCKED` |
| FI-07 | macOS GitHub-hosted qualification | `unknown` | Qualification artifact has no filesystem identity for local resource claim | `UNAVAILABLE` | Not applicable to local support claim | `NOT_APPLICABLE` |
| FI-14 | Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs`, CAS | `artifacts/m1-release-evidence/fault/linux-native-fi14-cas-20260828.json` + raw log | `NATIVE` | `PASS` for tmpfs row only | Overall matrix `BLOCKED` |
| FI-14 | Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs`, metadata | `artifacts/m1-release-evidence/fault/linux-native-fi14-metadata-20260828.json` + raw log | `NATIVE` | `PASS` for tmpfs row only | Overall matrix `BLOCKED` |
| FI-14 | Linux VM `rtc-node-1` | Dedicated 64 MiB `tmpfs`, journal | `artifacts/m1-release-evidence/fault/linux-native-fi14-journal-20260828.json` + raw log | `NATIVE` | `PASS` for tmpfs row only | Overall matrix `BLOCKED` |
| FI-14 | Linux x86_64 local support | Root `ext4` | No retained native ext4 row | `UNAVAILABLE` | Not executed | `BLOCKED` |
| FI-14 | Windows x86_64 local support | `NTFS` | No retained native quota/disk-full row | `UNAVAILABLE` | Not executed | `BLOCKED` |
| FI-14 | macOS GitHub-hosted qualification | `unknown` | No local filesystem/resource claim | `UNAVAILABLE` | Not applicable to local support claim | `NOT_APPLICABLE` |

The existing Linux FI-13 ext4 row is closed separately and is not rerun here:
`linux-fi13-posix-closure-20260829.json` records `NATIVE`, `errno=13`,
`PERMISSION_DENIED`, restoration, cold reopen, and exit code `0`.

## Remaining Technical Blockers

Only conditional technical work remains:

1. If Windows NTFS and Linux ext4 stay in the accepted M1 support scope,
   execute and retain safe native FI-07 rows for those filesystems.
2. If Windows NTFS and Linux ext4 stay in scope, execute and retain safe native
   FI-14 CAS/metadata/journal rows for those filesystems.
3. If the Owner keeps external side-effect execution in M1, provide the real
   FI-03 provider/interruption/reconciliation harness and evidence.

No production-code change is required by this scope closure. No property
corpus rerun is required by this document.

## Owner Decisions

The Release Owner must decide and record:

1. Whether FI-03 external side-effect execution is excluded from M1 (deferred
   to M2+) or remains a release requirement.
2. Which local platform/filesystem and resource models are actually accepted;
   unmeasured FI-07/FI-14 rows must be tested or explicitly excluded with a
   bounded exception.
3. Whether the proposed one-corpus property policy and qualification rows are
   accepted.
4. The residual-risk disposition and final M1 sign-off. Codex does not sign a
   waiver, accept an ADR, or create a release tag.

## Final Status

```text
FI-03 = DEFERRED_PENDING_OWNER_DECISION
FI-07 = PASS for retained Linux tmpfs row only; required ext4/NTFS rows missing
FI-14 = PASS for retained Linux tmpfs rows only; required ext4/NTFS rows missing
FAULT_MATRIX = BLOCKED
PROPERTY_TESTING = NOT_PROVEN
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```

This document is a scope/evidence closure artifact only. It does not enter M2,
create a release candidate, create a tag, or generate `M1_FINAL_GATE_REPORT.md`.
