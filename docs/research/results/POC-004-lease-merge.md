# POC-004: Two-Agent Lease and Merge

- Status: Pass for the modeled optimistic policy; production gate remains open.
- Run date: 2026-08-19
- Harness: `D:/pong/poc/run_phase1_pocs.mjs`
- Runtime: Node.js v22.17.0
- Sample: 1,000 deterministic iterations

## Results

- Stale lease publication was rejected in all 1,000 iterations.
- Disjoint file changes merged deterministically in all 1,000 iterations.
- Overlapping changes produced explicit conflicts in all 1,000 iterations.
- No modeled change was silently lost or resolved by last-writer-wins.

## Limitations and decision

The harness does not implement real filesystem leases, process crashes, branch refs, or non-file resources. It validates the expected policy and merge function only. Keep ADR-0004/0010 unchanged; M1/M2 must add real CAS ref, lease expiry, crash takeover, and resource-specific conflict tests before collaboration is production-ready.

