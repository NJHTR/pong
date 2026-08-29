# M1 Fault Scope Decision

**Prepared:** 2026-08-28  
**Audited commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Decision authority:** Release Owner  
**Current Gate:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`

**Owner Decision Draft:** `APPROVED_FOR_FINALIZATION`  
**Formal Owner acceptance:** `PENDING`

> **Final acceptance overlay (2026-08-29):** Owner `NJHTR` accepted FI-03 as
> `DEFERRED TO M2+`, FI-07/FI-14 only for the retained Linux `tmpfs` resource
> model, and FI-13 for Windows NTFS plus Linux ext4. The pending statements
> below are historical proposal context. Candidate source commit `58e9e875...`
> is frozen; final CI remains pending.

This document proposes the M1 fault scope without changing the existing fault
matrix or claiming unexecuted evidence. A proposed disposition becomes
effective only when the Release Owner records it in the sign-off/acceptance
records.

## Scope Rules

- M1 is a durability, correctness, and recovery-focused release baseline.
- Synthetic failpoints remain valid implementation evidence and future
  integration groundwork; they are never relabeled as external-provider
  evidence.
- FI-07 and FI-14 are required where the supported filesystem/resource model
  makes the scenario applicable.
- FI-13 has retained Windows NTFS/ACL evidence. The normative M1 gate still
  requires permission/read-failure behavior on every accepted local platform;
  Linux POSIX/ACL qualification is therefore required if Linux remains in the
  supported scope. No Linux FI-13 test is added by this scope document.
- FI-03 is deferred because M1 does not qualify external provider side
  effects. This is a scope exclusion proposal, not a claim that FI-03 passed.

The retained Linux `tmpfs` FI-14 and related FI-07 records do not generalize
to the proposed Linux ext4 or Windows NTFS rows. For every resource model the
Owner accepts, FI-07/FI-14 must either receive its own controlled evidence or
be named explicitly as an excluded row with a bounded residual-risk statement.
This proposal does not mark those unmeasured rows `PASS`.

## Fault Disposition

| Fault | Proposed M1 disposition | Scope and evidence | Residual risk / condition |
| --- | --- | --- | --- |
| FI-01 | `REQUIRED` | Durable intent boundary, synthetic failpoint, and cold reopen evidence | Native power-loss qualification remains outside this proposal unless separately accepted |
| FI-02 | `REQUIRED` | Durable-intent process boundary and unknown preservation evidence | External tool execution is not qualified |
| FI-03 | `DEFERRED` | Real external tool/provider effect plus abrupt termination is not executable in current Core; PT-10 synthetic schedules are retained as groundwork | No M1 claim for external side-effect reconciliation; M2+ Agent Integration must add the provider hook and evidence |
| FI-04 | `REQUIRED` | WAL tail/truncation and recovery evidence | Native journal/power-loss behavior remains unclaimed |
| FI-05 | `REQUIRED` | Outcome-durable outbox and idempotent publication evidence | Broader external process matrix remains unqualified |
| FI-06 | `REQUIRED` | Metadata transaction pre/post visibility evidence | Native power-loss ordering remains unclaimed |
| FI-07 | `REQUIRED` | Linux disposable `tmpfs` CAS ENOSPC evidence; each additional supported resource model requires its own row | Windows native quota/disk-full is not claimed unless separately evidenced or excluded |
| FI-08 | `REQUIRED` | CAS publication/directory-sync and cold reopen evidence | Filesystem-specific durability remains scoped to exercised rows |
| FI-09 | `REQUIRED` | Ref CAS, retry, and idempotency evidence | Distributed transport is outside M1 |
| FI-10 | `REQUIRED` | Projection rebuild, generation binding, repeated crash/reopen evidence | Final acceptance depends on ADR-0016 owner decision |
| FI-11 | `REQUIRED` | Redaction and repository-tree secret scan evidence | External export integrations remain outside M1 |
| FI-12 | `REQUIRED` | Generation migration and old-or-new visibility evidence | Historical old-reader compatibility is separately dispositioned |
| FI-13 Windows | `REQUIRED` | Windows x86_64 / NTFS / ACL revocation is the M1 permission qualification; raw error 5 and `PERMISSION_DENIED` evidence retained | Does not generalize to POSIX or other filesystems |
| FI-13 Linux | `REQUIRED` | If Linux remains accepted, a Linux chmod/chown/ACL row is required; none is currently retained | May become `NOT_APPLICABLE` only if the Owner removes Linux from M1 support |
| FI-14 | `REQUIRED` | Linux `tmpfs` CAS/metadata/journal `errno=28` and `RESOURCE_EXHAUSTED` evidence retained; each additional accepted resource model needs its own row | Each additional accepted filesystem/resource model needs its own evidence |

`OPTIONAL` is reserved for future provider-specific or filesystem-specific
qualification outside the required M1 baseline. No current FI row is silently
promoted to `PASS` by this document. `WAIVED` is not assigned by Codex; it is
available only if the Owner signs an explicit exception with scope and risk.

## FI-03 Deferred Boundary

The current Pong Core has no independent external provider/tool execution
hook. A local synthetic failpoint can prove that an unresolved intent remains
`unknown`, but cannot prove that a real provider effect was reconciled after a
process termination. M1 therefore makes no external-side-effect safety claim.
The synthetic tests remain in the repository as groundwork for M2+ Agent
Integration and must remain clearly marked synthetic.

## Linux FI-13 Boundary

The existing permission test uses Windows `icacls`, NTFS deny ACE semantics,
Windows error 5, ACL restoration, and cold reopen. The current M1 proposal
does not add a Linux equivalent. Because the proposed platform scope includes
Linux ext4, Linux FI-13 remains a missing required row. It can be recorded as
`NOT_APPLICABLE FOR M1` only after the Owner removes Linux from the supported
M1 platform scope.

## Acceptance

The Owner must record the effective fault scope, the FI-03 deferral, the Linux
FI-13 disposition, and residual risks in the release sign-off. Until then the
machine-readable matrix remains `not_passed`, and M1 remains blocked.

## Scope Freeze Classification (Proposed)

The table below is the proposed M1 classification requested for scope freeze.
It is not an owner waiver and does not change `artifacts/m1-fault-matrix.json`.
`REQUIRED` means required for every platform/filesystem row that the Owner
accepts into M1. `DEFERRED` and `NOT_APPLICABLE` are conditional dispositions
that become effective only after the Owner records the corresponding scope
decision. No row is assigned `WAIVED` by Codex.

### M1 Supported Platform Scope (Proposed)

1. Windows x86_64 on NTFS.
2. Linux x86_64 on ext4, with Linux-native VM evidence from Debian 13 and
   GitHub-hosted Linux evidence retained separately.
3. A GitHub-hosted macOS runner with filesystem `unknown`. This is not a
   physical-Mac or APFS claim.

Docker overlay, Docker-backed ext4 volumes, and the dedicated Linux `tmpfs`
mount are evidence environments only. They are not additional supported
platform/filesystem claims unless the Owner explicitly adds them.

### Required Fault Matrix

| Fault | Proposed disposition | Platform / filesystem boundary | Fault mechanism | Evidence class |
| --- | --- | --- | --- | --- |
| FI-01 | `REQUIRED` | Every accepted local platform/filesystem row | Before intent durability; pre-state or retryable rejection | `SYNTHETIC` plus accepted-row qualification |
| FI-02 | `REQUIRED` | Every accepted local platform/filesystem row | After intent durability, before execution | `SYNTHETIC` / `NATIVE` where retained |
| FI-03 | `DEFERRED` (only if external effects are excluded) | No M1 external-provider claim; M2+ provider boundary | After real tool effect, before outcome append | `SYNTHETIC` only today; required `EXTERNAL` evidence is absent |
| FI-04 | `REQUIRED` | Every accepted local platform/filesystem row | Outcome append or torn WAL tail | `SYNTHETIC` / `NATIVE` where retained |
| FI-05 | `REQUIRED` | Every accepted local platform/filesystem row | After durable outcome, before publication/ref transaction | `SYNTHETIC` / `NATIVE` where retained |
| FI-06 | `REQUIRED` | Every accepted local platform/filesystem row | Metadata transaction commit boundary | `SYNTHETIC` / `NATIVE` where retained |
| FI-07 | `REQUIRED` | Each accepted resource/filesystem model; retained row is Linux `tmpfs` | CAS short write or ENOSPC during staging | `NATIVE` for retained Linux VM row; `SYNTHETIC` elsewhere |
| FI-08 | `REQUIRED` | Every accepted local platform/filesystem row | After CAS publication, before directory durability | `SYNTHETIC` / `NATIVE` where retained |
| FI-09 | `REQUIRED` | Every accepted local platform/filesystem row | After ref CAS, before response | `SYNTHETIC` / `NATIVE` where retained |
| FI-10 | `REQUIRED` | Every accepted projection row | Projection rebuild handler/checkpoint/verification/publication boundaries | `NATIVE` current-host; `GITHUB_HOSTED` focused runner evidence |
| FI-11 | `REQUIRED` | Every accepted local platform/filesystem row | Redaction or export boundary | `SYNTHETIC` / `NATIVE` security scans |
| FI-12 | `REQUIRED` | Every accepted generation/migration row | Schema/generation migration interruption | `NATIVE` current-host and Linux VM; `GITHUB_HOSTED` focused evidence |
| FI-13 (Windows) | `REQUIRED` | Windows x86_64 / NTFS ACL semantics | ACL/permission revocation and read failure | `NATIVE` Windows NTFS evidence retained |
| FI-13 (Linux) | `REQUIRED` | Linux ext4 is a proposed local row, but no chmod/chown/ACL entry is retained | Permission denial must be fail-closed and recover after restoration | `UNAVAILABLE` until a Linux row is executed, or `NOT_APPLICABLE` only after Linux exclusion |
| FI-13 (hosted macOS qualification) | `NOT_APPLICABLE` (proposed) | Hosted macOS has no claimed local filesystem/permission support row | No separate local permission claim is made for the hosted qualification runner | `UNAVAILABLE` for a local permission claim |
| FI-14 | `REQUIRED` | Each accepted resource/filesystem model; retained row is Linux `tmpfs` | Disk full during journal, object, or metadata write | `NATIVE` for retained Linux VM rows; `EXTERNAL` for retained Docker rows |

`OPTIONAL` has no assigned M1 fault row at this time. `WAIVED` also has no
assigned row: an Owner may sign a bounded exception for a row only where the
release scope explicitly excludes the affected capability, platform, or
filesystem. An exception does not relabel the underlying evidence as `PASS`.

### Evidence and Acceptance Boundary

- FI-03 is not a missing local test that can be solved by adding a fake
  provider. It requires a real external effect and is therefore either an
  explicitly deferred M2+ capability or a release blocker while that claim is
  made.
- Windows FI-13 is a Windows NTFS/ACL qualification. Linux POSIX permission
  semantics are not silently inferred from it.
- FI-07 and FI-14 retained Linux `tmpfs` rows pass only for that disposable
  resource model. They do not prove generic ext4 or Windows quota behavior.
- The current machine-readable fault matrix remains `not_passed` until the
  Owner records the effective scope and all required rows for that scope.

**Current effective status:** `OWNER_ACTION_REQUIRED`; no scope freeze has
been accepted and `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`.

## Owner Decision Draft Overlay

The supplied Owner Decision Draft narrows the effective M1 fault claim for
final-candidate preparation:

| Fault | Draft disposition | Evidence boundary |
| --- | --- | --- |
| FI-03 | `DEFERRED TO M2+` | No external provider/tool execution claim; synthetic groundwork only |
| FI-07 | `PASS` (bounded) | Linux dedicated `tmpfs` resource model only |
| FI-13 | `PASS` (bounded) | Windows NTFS ACL and Linux ext4 POSIX rows; macOS local row not applicable |
| FI-14 | `PASS` (bounded) | Linux dedicated `tmpfs` CAS/metadata/journal model only |

Unclaimed Windows NTFS, Linux ext4, and hosted-macOS resource models are
outside this M1 fault contract, not evidence passes for those models. Linux
FI-13 is now backed by the retained native record
`artifacts/m1-release-evidence/fault/linux-fi13-posix-closure-20260829.json`.
Formal scope acceptance and any compatibility exception remain pending.
