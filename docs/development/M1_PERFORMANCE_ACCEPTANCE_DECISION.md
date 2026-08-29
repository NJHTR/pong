# Pong M1 Performance Acceptance Decision

**Status:** `ACCEPTED AS INFORMATIONAL BASELINE`  
**Owner:** `NJHTR`  
**Date:** `2026-08-29`  
**Formal acceptance:** `ACCEPTED`  
**ADR:** `ADR-0015-m1-performance-capacity-budget.md` (remains `Proposed`)

```text
Performance evidence = PASS as measurement
Performance certification = NOT REQUIRED FOR M1
ADR-0015 = ACCEPTED AS INFORMATIONAL BASELINE
```

This document records the Release Owner's accepted informational-baseline
decision for final-candidate preparation. It does not modify the source ADR
file or create new thresholds.

## Measurement Evidence

The retained `m1-perf-0.3` package contains raw samples and p50/p95/p99/max
aggregates for repository open, metadata write, event append, recovery,
migration, CAS throughput, and recorded snapshot scales. It includes three
Windows stable runs, three Windows Rust 1.78 runs, and three pinned Linux
Docker-overlay runs, plus the retained close-out measurements.

The package is measurement evidence for the recorded workloads. Docker
overlay is not a native Linux-host claim, and no hosted macOS filesystem claim
is inferred from the quality workflow.

## Current Baseline

The recorded workloads are below the proposed ADR-0015 limits for the measured
samples. Representative values from the retained package include repository
open p95 `32.488 ms`, metadata intent/outcome p95 `0.328 ms`, event append p95
`0.257 ms`, recovery p95 `0.698 ms`, migration p95 `89.698 ms`, and CAS
throughput up to the recorded `14.669 MiB/s` sample.

These are observations, not an accepted capacity budget.

## M1 Informational Scope

The Release Owner sets M1 performance to:

```text
INFORMATIONAL BASELINE
NOT CAPACITY-CERTIFIED
```

The baseline is limited to the retained workloads and their declared
platform/filesystem environments. It must not be read as a guarantee for
unmeasured native Linux ext4, hosted-macOS filesystem, Windows quota/disk-full,
memory, CPU, or unlimited-scale behavior.

## Capacity Certification Deferred

Capacity qualification beyond the retained baseline is deferred to a later
phase. No historical measurement is modified, and no missing run is relabeled
as `PASS`.

## Owner Decision

| Decision item | Draft state | Formal state |
| --- | --- | --- |
| M1 performance policy | Informational baseline | `ACCEPTED` |
| ADR-0015 acceptance | Informational baseline; source ADR remains `Proposed` | `ACCEPTED` |
| Capacity certification | Deferred beyond M1 | `NOT REQUIRED FOR M1` |
| Owner / date / signature | NJHTR / 2026-08-29 / textual owner declaration | `ACCEPTED` |

The Release Owner accepted the informational baseline policy for the named
measured scope. This does not alter the ADR file or its proposed status.

No threshold, historical measurement, or outlier is changed or removed by this
decision record.
