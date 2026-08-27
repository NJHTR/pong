# POC-002: Capture Completeness

- Status: Pass for the toy support matrix; generic capture claims remain bounded.
- Run date: 2026-08-19
- Harness: `D:/pong/poc/run_phase1_pocs.mjs`
- Runtime: Node.js v22.17.0
- Sample: 1,000 lease/merge iterations; one deterministic filesystem corpus

## What was tested

The corpus exercised wrapped file creation, write, and move operations, then performed a direct bypassed write. The harness compared the ground-truth filesystem manifest with native records and emitted a `reconciled` record for the bypass. An unsupported web action was explicitly labeled `unsupported`/`missing`, never complete.

## Results

- Four controlled filesystem effects had corresponding native or reconciliation evidence.
- The bypassed write produced a visible reconciliation record.
- Unsupported network behavior was not overstated as captured.
- No silent omission occurred in this corpus.

## Limitations and decision

This experiment does not test kernel watchers, child processes, shell escapes, browser middleware, databases, or concurrent writers. It validates the confidence vocabulary and reconciliation shape only. ADR-0009 remains valid; v0.1 must publish a platform-specific capture matrix and repeat the corpus with real wrappers and crash injection.

