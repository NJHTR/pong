# M1 Final Gate Closure Plan

**Created:** 2026-08-28  
**Base HEAD:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Branch:** `dev`  
**Current gate:** `M1_RELEASE_GATE = BLOCKED`

The plan was executed through the available local and retained evidence. The
traceability row is now internally consistent for HEAD `6cb62fb`; all other
rows retain the dispositions below unless a real external artifact or owner
decision is added.

This plan is a closure checklist, not an approval. It preserves the rule that
implemented code, a local test, a simulated fault, or a successful CI job is
not sufficient evidence for a broader release claim.

| Blocker | Current evidence | Missing evidence | Required action | Owner | Acceptance criteria | Verification command | Expected artifact | Status |
|---|---|---|---|---|---|---|---|---|
| Old-reader compatibility | `compatibility/old-reader-audit.json` and `.log` explicitly record no reader, source, execution, or exit code; current-source v0.1 fixtures are supplementary | Independently released historical Pong v0.1 reader, exact version/commit, binary SHA-256, fixture, read probe, mutation/open probe, stdout/stderr and exit code | Locate a real historical reader. If none exists, keep the row blocked and obtain an owner-approved exception; never compile the current reader and call it old | Compatibility owner | A real binary independent of current reader code reads the current M1 fixture and fails closed on mutation, with retained machine-readable and raw output | Version command plus the exact reader invocation against the fixture; record binary hash | `artifacts/m1-release-evidence/compatibility/old-reader-*.{json,log}` and updated `old-reader-audit.json` | `BLOCKED` |
| Fault matrix FI-03 | Property/process crash schedule exists, but no real external tool-effect hook | Real tool effect followed by termination before outcome, provider/workspace reconciliation result | Execute the external provider/tool schedule in a disposable environment, or retain explicit unavailable disposition | Reliability/runtime owner | Unknown effect is never inferred as success; raw provider evidence and cold reopen are retained | External harness command plus restart/reconciliation inspection | Per-case FI-03 JSON/log and matrix entry | `BLOCKED` |
| Fault matrix FI-07 | Synthetic short-write/quota tests and Linux tmpfs CAS ENOSPC evidence | Complete declared-platform disk-full/quota matrix, especially Windows native and any accepted filesystem | Run controlled quota/ENOSPC cases without risking the development volume; retain OS error and restart inspection | Storage/platform owner | Partial CAS bytes unreachable, committed history readable, stable `RESOURCE_EXHAUSTED` classification | Host-resource commands in `tests/HOST_RESOURCE_FAULTS.md` | Per-platform FI-07 JSON/log and updated fault matrix | `BLOCKED` |
| Fault matrix FI-13 | Windows NTFS ACL read-revocation evidence exists | Complete FI-13 schedule on every declared/accepted filesystem | Execute permission revocation and recovery on each accepted row, or document an explicit owner-approved scope exception | Platform/reliability owner | Stable permission/quarantine/recovery behavior with raw OS error and cold reopen | Host-resource fault command plus restore/reopen inspection | Per-platform FI-13 JSON/log | `BLOCKED` |
| Fault matrix FI-14 | Linux disposable 64 MiB tmpfs CAS/metadata/journal evidence exists | Windows native disk-full/quota and any other accepted filesystem rows | Execute controlled resource exhaustion on every accepted row, or owner-approve a bounded exception | Storage/platform owner | `RESOURCE_EXHAUSTED`, no loss of committed history, valid cold reopen | Host-resource commands with isolated scratch volume | Per-platform FI-14 JSON/log | `BLOCKED` |
| Property testing | PT-01..PT-12 and PT-14 retained Windows stable corpus; native Run `33145714975` runs normal property tests | Normative property corpus records on every accepted platform/filesystem row; explicit PT-13 projection acceptance | Run the declared corpus on accepted rows or record a bounded owner exception; keep seed/case count/invariant metadata | Reliability owner | Each required property has generator/corpus, seed, case count, platform, result and retained raw output | Property test commands from the M1 evidence procedure | Per-row property JSON/log and matrix references | `NOT_PROVEN` |
| Event / Projection | ADR-0016 implementation; event/projection and PT-13/FI-10 suites pass locally and in native focused run | ADR-0016 acceptance decision plus independent versioned/golden fixture and accepted cross-platform evidence | Review contract against tests, add only any missing evidence fixture, then have owner accept or reject ADR-0016 | Core/release owner | Ordering, determinism, replay, duplicate, unknown/version behavior, generation identity and cold-reopen invariants are evidenced | `cargo test --locked --test event_projection`; `cargo test --locked --test pt13_fi10 -- --nocapture` | `M1_PROJECTION_ACCEPTANCE.md` or owner decision attached to ADR-0016 | `NOT_PROVEN` |
| Performance budget | `m1-perf-0.3` contains p50/p95/p99/max, CAS throughput, migration and snapshot scales; repeated Windows/Linux-overlay raw runs | ADR-0015 acceptance, three-run evidence for each accepted platform/filesystem row, memory/CPU or explicit scope decision | Use the prepared acceptance record; owner accepts thresholds/scope or records a compensating exception | Release owner | ADR-0015 decision is recorded with accepted rows, thresholds, run IDs and exceptions | `cargo test --locked --test performance_measurements -- --ignored --nocapture` per accepted row | `docs/development/M1_PERFORMANCE_ACCEPTANCE.md` and ADR-0015 acceptance table | `NOT_PROVEN` |
| Traceability | Run `33145714975` artifacts and regenerated traceability identify commit `6cb62fb`; release bundle checksums and matrix are internally consistent | No release candidate tag; working tree contains uncommitted evidence/documentation changes; owner approval remains pending | Bind the final clean evidence snapshot to the owner-controlled release candidate commit/tag; do not create a formal release tag here | Release/repository owner | Source commit -> workflow run -> platform artifacts -> fault/property/performance/compatibility -> final report is one consistent chain | `cargo test --locked --test artifact_consistency -- --nocapture` plus traceability inspection | Updated `traceability/release-traceability.json` and `.log` | `BLOCKED` (release) - current snapshot verified |
| Release-owner sign-off | Existing records intentionally say `UNASSIGNED` / `PENDING` | Named owner, date, accepted scope, limitations, risks, decision and signature | Owner reviews final bundle and fills the sign-off template; the agent must not self-approve | Release owner | Human decision is `ACCEPTED` or `REJECTED` with signature/date; no unresolved required blocker is hidden | Manual review of bundle and signed record | `docs/development/M1_RELEASE_OWNER_SIGNOFF.md` | `BLOCKED` |

## Closure Order

1. Keep old-reader, FI-03/FI-07/FI-13/FI-14, and owner rows blocked until their
   real external evidence exists.
2. Record the performance and Projection acceptance decisions without changing
   thresholds to fit existing measurements.
3. Regenerate traceability only after the final evidence snapshot is known.
4. Run the complete quality gates and all M1 focused suites against the exact
   release candidate commit.
5. Generate `M1_FINAL_GATE_REPORT.md` only after the technical rows are closed
   or explicitly accepted by the release owner. Until then the report must not
   claim `PASS`.

## Current Decision

```text
M1_RELEASE_GATE = BLOCKED
OLD_READER = BLOCKED
FAULT_MATRIX = BLOCKED
PROPERTY_TESTS = NOT_PROVEN
EVENT_PROJECTION = NOT_PROVEN
PERFORMANCE = NOT_PROVEN
TRACEABILITY = BLOCKED (release; current snapshot is internally consistent)
RELEASE_OWNER_SIGNOFF = BLOCKED
```

The next human-required inputs are the real old-reader artifact (or a formal
compatibility exception), accepted fault/platform scope, ADR-0015/ADR-0016
decisions, and release-owner sign-off. No M2 work is authorized by this plan.
