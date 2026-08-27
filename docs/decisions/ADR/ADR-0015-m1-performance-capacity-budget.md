# ADR-0015: M1 Performance and Capacity Budget

- Status: Proposed; release-owner acceptance pending
- Date: 2026-08-26

## Context

M1 has a reproducible measurement probe, but a measurement is not a capacity
contract. Without a bounded workload and a declared threshold, a green local
run cannot support a release claim. The current `m1-perf-0.3` probe records raw
samples and p50/p95/p99/max for repository open, metadata intent/outcome, event
append, recovery, and eight independent migration runs, plus CAS throughput and
three local snapshot sizes in `artifacts/m1-performance.json`.

The values below are deliberately workload-scoped. They are not a promise that
Pong can handle an unlimited repository, workspace, or artifact set. They must
be remeasured on every accepted platform/filesystem row in
[`M1_COMPATIBILITY_MATRIX.md`](../../development/M1_COMPATIBILITY_MATRIX.md).

## Proposed budget

| Operation | Fixed workload for acceptance | Proposed threshold |
| --- | --- | --- |
| Repository open | cold open of a healthy repository with <= 256 KiB metadata and <= 1,000 CAS directory entries | p95 <= 100 ms, p99 <= 250 ms |
| Metadata write | one intent plus one outcome, 64 repetitions, FULL WAL | p95 <= 10 ms, p99 <= 25 ms |
| Event append | one canonical event append, 64 repetitions | p95 <= 10 ms, p99 <= 25 ms |
| Recovery | classify 64 unfinished intents after cold open | p95 <= 100 ms, p99 <= 250 ms |
| CAS publication | 32 distinct 16 KiB objects | throughput >= 1 MiB/s and zero partial readable objects after a failed write |
| Repository migration | v0.1 source with <= 256 KiB metadata to v0.2 generation | p95 <= 1 s, p99 <= 3 s; selector old/new invariant must remain green |
| Local snapshot (M2 baseline) | 4 x 1 KiB, 32 x 4 KiB, and 128 x 16 KiB regular files | record p95 per scale; no release threshold until M2 total-byte budget exists |

The failure-safety part of the budget is mandatory even when a timing budget is
missed: committed history remains readable, no partial CAS object is reachable,
and a migration interruption exposes only the old or fully verified new
generation.

## Measurement and acceptance protocol

1. Run the fixed probe three times per platform/filesystem with a quiet host,
   preserving raw samples, toolchain, lockfile hash, filesystem type, and
   repository sizes.
2. Use the median run for the report and retain all runs; do not hide a failed
   run by rerunning until it is green.
3. Repeat the host-fault tests separately. A timing pass never overrides an
   integrity, redaction, permission, quota, or recovery failure.
4. The release owner accepts this ADR only after every declared platform row
   has three successful runs or an explicit exception with a compensating
   bound and owner.

## Current Measurement Observation

The 2026-08-26 raw-run package contains three Windows stable, three Windows
Rust 1.78, and three pinned Linux Docker-overlay runs. The following are the
per-run extrema across those files, not an accepted budget:

| Metric | Observed range | Proposed limit |
| --- | --- | --- |
| Repository open p95 / p99 | 0.775-39.469 ms / 0.849-46.623 ms | 100 ms / 250 ms |
| Metadata write p95 / p99 | 0.350-1.500 ms / 0.403-2.242 ms | 10 ms / 25 ms |
| Event append p95 / p99 | 0.205-0.877 ms / 0.241-1.137 ms | 10 ms / 25 ms |
| Recovery p95 / p99 | 0.760-1.574 ms / 0.752-1.574 ms | 100 ms / 250 ms |
| Migration p95 / p99 | 15.388-102.522 ms / 15.388-102.522 ms | 1 s / 3 s |
| CAS throughput | 10.356-14.509 MiB/s | >= 1 MiB/s |

These observations cover Windows NTFS and Docker overlay only. They do not
cover native Linux ext4, Windows native disk-full, macOS, or any other
unaccepted filesystem row. The raw files and SHA-256 manifest are retained in
[`artifacts/m1-performance-runs/`](../../../artifacts/m1-performance-runs/).

## Acceptance record

| Field | Current value |
| --- | --- |
| Release owner | Unassigned |
| Decision | Proposed; not accepted |
| Decision date | Not recorded |
| Accepted platform/filesystem scope | None |
| Exceptions | None recorded |
| Required retained evidence | Three raw runs per accepted matrix row, toolchain/lockfile/filesystem identity, and failure-safety results |

This record is intentionally pending. A measurement artifact can support a
decision, but it does not accept the thresholds or widen the supported-platform
claim by itself. The owner must update this table and the M1 evidence register
together; until then this ADR remains non-binding and M1 remains `NOT PASSED`.

## Capacity boundaries

The current M1 implementation has no general total-repository byte quota and
must not claim one. CAS/object and metadata limits are host-resource behavior;
the real Linux `tmpfs` and Windows ACL tests establish failure classification,
not a supported capacity. M2 currently limits file count and per-file size but
does not yet enforce a total snapshot-byte budget. A total-byte limit and a
large-tree measurement are required before snapshot capacity can be accepted.

## Consequences

- Performance becomes a measurable release condition instead of an informal
  expectation.
- The thresholds are intentionally conservative relative to the current
  Windows probe, while remaining finite and falsifiable.
- A platform that cannot meet the budget is unsupported for the release or
  requires a new ADR; it is not silently exempted.
- This ADR does not add a public API, CLI, Runtime, SDK, server, or framework
  adapter.

## Decision

**Not accepted yet.** The proposal is the next acceptance artifact. M1 remains
`NOT PASSED` until the release owner accepts the platform/old-reader matrix,
these budgets, the complete fault disposition, and the broader
directory-sync/filesystem coverage required by the PT-14 evidence.
