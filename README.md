# Pong

Pong is Git-like version control and coordination infrastructure for AI-agent execution. Phase 0 established the architecture baseline; the repository now contains an internal, test-gated Rust durable-primitives core plus bounded M2 workspace/snapshot and M3 operation-ledger slices.

Start with [`docs/README.md`](docs/README.md), then read [`docs/roadmap/NEXT_TASK.md`](docs/roadmap/NEXT_TASK.md). The repository has no public CLI, runtime interception, SDK, server, or UI. M1 is not a passed release gate, and the M2/M3 code is not a completed or released provider/runtime contract. Rust modules are currently visible to integration tests, but are unreleased implementation surface; direct metadata/CAS constructors are unsupported and do not replace `Repository::open` identity validation.
