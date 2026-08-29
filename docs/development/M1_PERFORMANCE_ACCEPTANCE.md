# M1 Performance Acceptance Record

**Status:** `PENDING`  
**ADR:** [`ADR-0015-m1-performance-capacity-budget.md`](../decisions/ADR/ADR-0015-m1-performance-capacity-budget.md)  
**Prepared:** 2026-08-28

This record summarizes measurements; it does not accept the proposed budget.
Only the release owner may change the decision to `ACCEPTED` or record an
explicit bounded exception.

## Evidence Reviewed

The `m1-perf-0.3` package under
`artifacts/m1-release-evidence/performance/` retains raw samples and hashes for
three Windows stable runs, three Windows Rust 1.78 runs, and three pinned Linux
Docker-overlay runs. It also retains close-out Windows runs. The package does
not contain the required three-run set for native Linux ext4, native macOS, or
Windows native disk-full/quota.

Representative aggregate measurements from `m1-performance.json`:

| Operation | Samples | p50 | p95 | p99 | Max | Proposed ADR-0015 threshold |
|---|---:|---:|---:|---:|---:|---|
| Repository open | 64 | 22.316 ms | 32.488 ms | 38.578 ms | 41.425 ms | p95 <= 100 ms; p99 <= 250 ms |
| Metadata intent + outcome | 64 | 0.196 ms | 0.328 ms | 0.401 ms | 4.463 ms | p95 <= 10 ms; p99 <= 25 ms |
| Event append | 64 | 0.140 ms | 0.257 ms | 0.342 ms | 0.358 ms | p95 <= 10 ms; p99 <= 25 ms |
| Recovery mark intents unknown | 8 | 0.664 ms | 0.698 ms | 0.698 ms | 0.746 ms | p95 <= 100 ms; p99 <= 250 ms |
| Repository migration | 8 | 80.186 ms | 89.698 ms | 89.698 ms | 93.532 ms | p95 <= 1 s; p99 <= 3 s |
| CAS publication | 32 x 16 KiB | n/a | n/a | n/a | 14.669 MiB/s | >= 1 MiB/s |

The record contains no memory or CPU samples. Snapshot measurements are
recorded by scale, but ADR-0015 deliberately has no release threshold for the
M2 snapshot row yet.

## Acceptance Decision

| Field | Value |
|---|---|
| Release owner | Unassigned |
| Decision | `PENDING` |
| Decision date | Not recorded |
| Accepted platform/filesystem scope | None |
| Threshold changes | None proposed by this audit |
| Exceptions | None recorded |
| Required follow-up | Three successful runs per accepted row, then owner decision |

## Verification

The measurement probe is reproducible with:

```powershell
$env:PONG_PERF_OUTPUT = 'artifacts/m1-performance.json'
cargo test --locked --test performance_measurements -- --ignored --nocapture
```

Until the owner completes this record and ADR-0015, the performance Gate is
`NOT_PROVEN` and M1 cannot be marked `PASS`.
