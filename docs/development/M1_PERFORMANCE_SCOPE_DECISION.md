# M1 Performance Scope Decision

**Prepared:** 2026-08-28  
**Audited commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**ADR:** [`ADR-0015-m1-performance-capacity-budget.md`](../decisions/ADR/ADR-0015-m1-performance-capacity-budget.md)  
**Current status:** `OWNER_ACTION_REQUIRED`

**Owner Decision Draft:** `INFORMATIONAL BASELINE / APPROVED_FOR_FINALIZATION`  
**Formal acceptance:** `PENDING`

> **Final acceptance overlay (2026-08-29):** Owner `NJHTR` accepted
> `ADR-0015 = ACCEPTED AS INFORMATIONAL BASELINE`. The source ADR remains
> `Proposed`; capacity certification is explicitly outside M1. The pending
> labels below are retained proposal history.

This document summarizes the scope decision needed for ADR-0015. It does not
change the proposed thresholds or change ADR-0015 from `Proposed` to
`Accepted`.

## M1 Performance Baseline

For the current release-scope proposal, M1 performance is treated as
**measurement plus baseline**, not as an automatically hard pass/fail claim.
The retained `m1-perf-0.3` package is the baseline for the recorded workloads
and platforms. This interpretation does not remove or alter any numeric
threshold proposed by ADR-0015. Because ADR-0015 is still `Proposed`, the
Release Owner must choose whether to:

1. accept the measurements as an informational M1 baseline with explicit
   workload and platform limits; or
2. adopt the proposed thresholds as a release condition after deciding the
   accepted rows and any compensating exceptions.

Until that choice is recorded, the status remains `OWNER_ACTION_REQUIRED`.
An informational-baseline choice must still be recorded as a bounded
Owner-approved exception (with named workloads, excluded rows, and residual
risk) if ADR-0015 is not accepted as a release threshold. Keeping the ADR
`Proposed` without that record does not satisfy the M1 Gate.

## Measured

The retained `m1-perf-0.3` package contains raw samples and p50/p95/p99/max
for repository open, metadata write, event append, recovery, migration, and
CAS throughput. It contains three Windows stable runs, three Windows Rust
1.78 runs, and three pinned Linux Docker-overlay runs, plus supplemental
close-out measurements. Representative observed values are below the
proposed limits in the recorded workloads.

The measurements are evidence, not a capacity contract. The package does not
contain the required three-run set for native Linux ext4, the GitHub-hosted
macOS row, or Windows native disk-full/quota. It also contains no memory or
CPU samples, and ADR-0015 deliberately has no release threshold for the M2
snapshot row yet.

## Required by ADR-0015

1. Run the fixed probe three times for each platform/filesystem row accepted by
   the Release Owner.
2. Retain all raw runs, toolchain and lockfile identity, filesystem type,
   repository sizes, and operation scales.
3. Use the median run for the report; do not hide a failed run by rerunning
   until it is green.
4. Keep failure-safety results separate from timing results.
5. Record an accepted threshold or a compensating, bounded exception in the
   ADR and the acceptance record.

## Missing

| Scope item | Current evidence | Missing |
| --- | --- | --- |
| Windows NTFS stable/MSRV | Three-run packages retained | Owner acceptance |
| Linux Docker overlay | Three pinned runs retained | Owner acceptance; not a native-host claim |
| Linux native ext4 | Linux-native quality evidence exists | Three performance runs |
| GitHub-hosted macOS | Quality evidence exists; filesystem `unknown` | Three performance runs and owner decision on unknown filesystem |
| Windows native disk-full/quota | No safe local run | Disposable quota environment or explicit scope exclusion |
| Memory / CPU | No measurements and no ADR threshold | Owner decision whether these are outside M1 |
| Threshold decision | ADR remains `Proposed` | Accept thresholds or record a compensating bound |

## Owner Decision

The Release Owner must choose one of the following, without changing the
numeric thresholds merely to fit existing measurements:

- accept ADR-0015 for a named platform/filesystem and workload scope after the
  required runs are present;
- accept a bounded capacity scope with explicit compensating controls and
  excluded rows/metrics; or
- reject/defer the performance contract, which keeps M1 release blocked.

## Waiver

A bounded performance/capacity exception is possible only when it names the
excluded rows and metrics, operational limits, residual risk, owner, date, and
exact commit. It does not turn missing measurements into `PASS` and does not
permit an unlimited-scale claim.

**Current acceptance:** `PENDING`. The implementation agent must not accept
ADR-0015 or write a waiver on behalf of the owner.

## Owner Decision Draft Overlay

The supplied Owner Decision Draft selects the informational M1 performance
baseline and defers capacity certification. Existing `m1-perf-0.3` measurements
remain unchanged and are sufficient as recorded baseline evidence for this
bounded policy. ADR-0015 remains `Proposed`; formal acceptance or a bounded
exception with explicit limits is still required.
