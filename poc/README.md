# Pong Experimental PoCs

**EXPERIMENTAL / NOT PRODUCTION CODE.**

This directory contains disposable architecture-validation experiments for Phase 1. The scripts use only the Node.js standard library, are not imported by Pong, and do not define production APIs or storage formats. They model the documented safety properties and must be replaced by contract, property, and failure-injection tests before implementation.

Run from the repository root:

```text
node poc/run_phase1_pocs.mjs
```

The output is a JSON summary. The associated design, thresholds, limitations, and ADR routing rules are in [`docs/research/POC_EXECUTION_PLAN.md`](../docs/research/POC_EXECUTION_PLAN.md). A passing PoC does not establish a production guarantee.

