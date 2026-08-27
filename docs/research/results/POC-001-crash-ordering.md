# POC-001: Crash Ordering and Recovery

- Status: Pass for the modeled safety properties; production gate remains open.
- Run date: 2026-08-19
- Harness: `D:/pong/poc/run_phase1_pocs.mjs`
- Runtime: Node.js v22.17.0, Windows local filesystem
- Sample: 1,000 repetitions per modeled failpoint

## What was tested

The harness modeled the documented sequence `intent -> tool side effect -> outcome -> projection -> ref CAS` and dropped non-fsynced records at each boundary. Recovery was run twice for every case. A filesystem `fsync` smoke check verified that the host runtime can flush a journal file.

## Results

The clean path completed with `succeeded`, a durable projection, and a durable ref before the failure cases were evaluated.

| Failpoint | Recovered state |
| --- | --- |
| Before intent | `not_started`, no side effect, no ref |
| Intent before fsync | `not_started`, no side effect, no ref |
| Intent fsynced before tool | `unknown`, no side effect, no ref |
| Side effect before outcome | `unknown`, side effect visible, no ref |
| Outcome before fsync | `unknown`, side effect visible, no ref |
| Outcome fsynced before projection | `succeeded`, projection and ref repaired |
| Projection before ref CAS | `succeeded`, ref repaired idempotently |

All modeled recoveries were idempotent. No ref was published without a durable successful outcome, and no observed side effect was silently classified as success.

## Limitations and decision

This is a deterministic state-machine model, not a power-loss test or a real database/WAL implementation. It does not prove filesystem or SQLite crash behavior. Keep ADR-0007/0008/0012 unchanged, carry these cases into M1 failure-injection tests, and require an actual crash harness before production durability claims.
