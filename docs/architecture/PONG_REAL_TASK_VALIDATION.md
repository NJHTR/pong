# Pong Real-Task Validation

**Date:** 2026-10-05
**Baseline:** `c28e367` (`dev`)
**Status:** `REAL-TASK INTERNAL VALIDATION / NO IMPLEMENTATION AUTHORIZED`

## 1. Research Question

Does provider-neutral durable Agent Execution State save enough recovery and
provider-switching cost on a real engineering task to justify the additional
Pong setup and operating complexity?

This is an internal validation of real Pong engineering work. It is not a
market study, user-interview result, or claim that ordinary developers need
Pong. No new provider run or project modification was performed for this
document. The evidence is limited to workflows already executed and recorded
in the repository.

## 2. Real Tasks

Only three real repository tasks were selected. The first two are end-to-end
engineering workflows. The third is a real acceptance defect, included as a
failure and recovery case rather than as a product-demand claim.

| Task | Real activity | Evidence | Qualification |
| --- | --- | --- | --- |
| 1. M4-021 provider handoff | User-controlled Codex -> Checkpoint -> Claude continuation, fresh process, and cold reopen | `M4_021_FINAL_ACCEPTANCE_2026-10-05.md`, `m4-021-final-acceptance-2026-10-05.json` | Real cross-provider workflow; W1 content itself was small and controlled, so not proof of high-cost demand |
| 2. M4-020 native regression gate | Ubuntu 24.04 and macOS 14 native gates, artifact/evidence correction, and final green run | `M4_020_CURRENT_HEAD_NATIVE_RESULT.md`, run `37185403135` record | Real cross-platform engineering operation; GitHub Actions already solves most of the workflow |
| 3. M4-021 identity assertion defect | Cold-reopen harness compared `Workspace.head` with `snapshot_id` instead of `root_digest`; test-only correction followed | `M4_021_EXECUTION_RESULT_2026-10-04.md`, execution artifact | Real complex acceptance/debugging case; not independent evidence of market demand |

No accessible repository evidence represents a large production migration,
performance investigation, security remediation, or architecture decision
performed through Pong. Such a task must not be invented from the existing
calculator acceptance workflow.

## 3. Baseline Workflow

Baseline A is the workflow a professional engineer can use without Pong:

```text
Git branch or worktree
  -> Codex / Claude session
  -> commit or checkpoint note
  -> transcript / README / issue handoff
  -> new session or provider
  -> inspect files and rerun tests
  -> commit, PR, or revert
```

This baseline is intentionally practical. It does not assume that a user
maintains a second orchestration system by hand. Git already provides content
isolation, rollback, diff, merge, and history; provider tools provide sessions,
tool execution, and often task-level continuation.

### Task 1 baseline

Without Pong, Codex would commit W1, write a handoff note containing the
workspace and intended continuation, and Claude would inspect the checkout,
read the note, rerun relevant tests, and continue. A fresh process would rely
on the commit, files, transcript, and human explanation. No controlled baseline
run was executed, so reconstruction time and repeated work are **not measured**.

### Task 2 baseline

The actual baseline is the existing GitHub Actions native workflow: checkout a
known SHA, run fmt/check/clippy/focused tests/full regression, upload logs, and
review the artifact. This workflow is already appropriate. Pong is not needed
to compile or test the repository on Ubuntu or macOS.

### Task 3 baseline

The baseline failure mode is a test harness that conflates two identifiers. A
normal test repair reads the Core API, changes the assertion, and reruns static
checks. Git diff and test output already provide the needed review trail.

## 4. Pong Workflow

Candidate B uses the existing durable Core, without a new fork API:

```text
Agent
  -> Execution and Workspace ownership
  -> Version publication
  -> Checkpoint
  -> provider/process interruption
  -> Resume or Handoff into a new Execution
  -> independent provider continuation
  -> fresh-process inspection
  -> Repository::open / cold reopen
```

The M4-021 record proves this concrete sequence:

```text
E1 execution-m4-real-codex (interrupted)
  -> C1 checkpoint-m4-real-codex
  -> handoff-m4-real-codex-claude
  -> E2 execution-m4-real-claude (completed)
  -> C2 checkpoint-m4-real-claude
  -> fresh process
  -> Repository::open
  -> cold reopen
```

`Workspace.head` was verified against `snapshot.root_digest`, while Snapshot
identity was verified against `snapshot_id`. This distinction is part of the
durable fact set, not a provider transcript.

## 5. Time / Complexity Comparison

The repository does not contain a controlled A/B stopwatch for these tasks.
The honest result is therefore:

```text
Measured Pong savings (X): NOT RECORDED
Measured Pong added cost (Y): NOT RECORDED
Measured net value (X - Y): NOT PROVEN
```

No number is fabricated from command duration, provider latency, or test
runtime. The following table records what was and was not observed.

| Task | Setup time | Recovery/context time | Provider switch cost | Lost work | Manual steps | Restored-state confidence | Auditability |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M4-021 baseline | Not measured | Not run | Not measured | Unknown | Commit + handoff note + inspection | Unknown | Commit/transcript/notes |
| M4-021 Pong | Not measured | Not measured; recovery assertions passed | One explicit durable handoff; elapsed time not recorded | None observed in accepted run | Operator pauses, protocol operations, two user sessions | High for the asserted durable facts | Exact Task/Execution/Workspace/Version/Checkpoint/Handoff IDs |
| M4-020 baseline | Existing CI workflow | CI rerun/review | Not applicable | No Pong state involved | Workflow dispatch and artifact review | High for native gates | CI logs and SHA-bound artifacts |
| M4-020 Pong | Not applicable | No additional Pong recovery value observed | Not applicable | Not applicable | No Pong operation required | Same as CI evidence | Same CI evidence |
| Identity defect baseline | Not measured | Test-only repair | Not applicable | No production work lost | Read API semantics and fix assertion | Depends on test correctness | Git diff and test output |
| Identity defect Pong | Not measured | Assertion corrected; no time captured | Not applicable | No production work lost | Same test repair plus identity separation | High for the corrected assertion | Durable identity rule is explicit |

The current evidence supports a qualitative conclusion only: Pong can reduce
reconstruction risk in Task 1, but it has not demonstrated a measured time
advantage over a disciplined Git-plus-session handoff.

## 6. Provider Switch Comparison

### Ordinary Git + transcript

```text
1. Stop Agent A and commit or save files.
2. Write what changed, what remains, and which tests matter.
3. Start Agent B with the repository and handoff note.
4. Agent B inspects files, infers state, and reruns validation.
5. Human verifies that the inferred state matches the intended checkpoint.
```

This is workable. Its weak point is not file portability; it is the human
reconstruction of Execution ownership, checkpoint reason, lease/revision state,
and the exact durable outcome after a crash or provider change.

### Pong durable handoff

```text
1. Publish a Version and create C1.
2. Interrupt E1 with a durable outcome.
3. Create a Handoff and resume E2 from C1.
4. Materialize W2 and let Agent B continue independently.
5. Complete C2 and E2.
6. Close the process and reopen the repository.
7. Verify the durable records and identity rules.
```

M4-021 executed this flow with independent user-controlled Codex and Claude
sessions. It proves the state can be reopened without transferring hidden
provider context. It does not prove that users save a particular number of
minutes, nor that the workflow is frequent enough to justify Pong for all
teams.

## 7. Recovery Comparison

| Failure | Git + Agent response | Pong response | Evidence status |
| --- | --- | --- | --- |
| Provider unavailable before checkpoint | Retry or start another session; manual context transfer | No false Core success; resume only from an existing durable checkpoint | Provider 503 was classified as environment failure |
| Process exits after checkpoint | Inspect commit/files and reconstruct handoff | Inspect E1 outcome, C1, source Version, and resume E2 | M4-021 fresh-process PASS |
| Both provider sessions end | Reopen checkout and trust notes/transcript | Fresh protocol process and `Repository::open` inspect durable lineage | M4-021 cold reopen PASS |
| Wrong identity comparison | Test may fail or silently compare the wrong field | Explicit `root_digest` vs `snapshot_id` distinction catches the mismatch | Harness defect reproduced and corrected |
| Native CI artifact defect | Fix artifact record and rerun workflow | Pong adds no material value | M4-020 run history and final PASS |
| One route fails while another continues | Git worktrees isolate files | Existing leases, revisions, executions, and checkpoints can isolate state | Core composition tests; no real user fork measured |

## 8. Failure Scenarios

The real failures are important because they delimit the claim:

1. The first M4-021 provider attempt stopped on a real Claude service `503`
   before target continuation. This was a provider-environment failure, not
   evidence that Pong recovered a user task.
2. A later real-provider run completed the workflow but failed the final
   harness assertion because `Workspace.head` (`sha256:<digest>`) was compared
   with Snapshot identity (`snp-<digest>`). The Core semantics were correct;
   the test was not.
3. M4-020 native artifacts initially exposed an evidence-package schema defect
   and a macOS full-regression fixture observation. The final native run passed
   without production changes.

These failures show that durable state and evidence boundaries are useful for
debugging, but they do not by themselves establish product-market value.

## 9. Pong Moments

### Observed Pong Moment: provider handoff plus cold reopen

After Codex's E1 was interrupted, Claude resumed from C1 in a separate
workspace. After E2 completed and the protocol process stopped, a fresh process
and `Repository::open` recovered the exact E1/E2/C1/C2 lineage and distinct
Workspace heads. The user did not need to transfer hidden provider context.

That is the clearest concrete benefit observed in the repository:

```text
resume factual state, not chat memory
```

However, the baseline reconstruction time was not captured. This is a real
workflow moment, not a measured market or productivity result.

### Non-Pong moment: cross-platform gate

The M4-020 task was solved by GitHub Actions, native runners, and artifacts.
Pong was not required. This is a direct counterexample to broad positioning.

## 10. User Value

| Task size | Result |
| --- | --- |
| Small task | Pong value is low; normal Git and Agent workflow is sufficient |
| Medium task | Pong is optional; value depends on interruption and provider changes |
| Large/high-risk task | Potentially high value if handoff/recovery is repeated and reconstruction is expensive |

The accepted M4-021 run demonstrates technical value but uses a small W1
change. The repository has no controlled evidence for a large production task.
Therefore the value proposition is currently:

```text
technically credible, operationally plausible, user value not yet measured
```

## 11. Complexity Cost

Pong adds:

- Repository/bootstrap setup;
- Agent, Execution, Workspace, Version, Checkpoint, Handoff, Operation, lease,
  and revision concepts;
- protocol/operator coordination;
- evidence retention and identity semantics;
- new recovery and unknown-outcome failure modes.

For a small task, `Y` is predictably larger than any likely recovery saving.
For a long-running task, the tradeoff is unknown until a real baseline and
Pong run are timed side by side. This document therefore does not claim
`Y < X`.

## 12. Product Positioning

The evidence does not support a broad claim of “forkable Agent platform.” If
the direction continues, it should be narrowed to:

```text
Durable Cross-Provider Agent Handoff
for long-running, high-cost engineering work
```

`Portable Execution State` remains the underlying technical description;
`Forkable Execution`, Compare, and Promote are not validated product
requirements.

## 13. Verdict

```text
CONDITIONAL GO
```

Reason:

- Task 1 contains a genuine Pong Moment and proves a provider-neutral durable
  handoff workflow.
- Task 2 shows a real class of work where Pong adds no meaningful value.
- Task 3 shows durable identity semantics improve acceptance correctness, but it
  is not evidence of external user demand.
- No controlled time comparison proves that Pong saves more effort than it
  adds, and no high-cost production task has been run through both workflows.

This verdict authorizes only further evidence gathering. It does not authorize
production implementation, a new Slice, or Protocol changes.

## 14. Next Recommendation

Do one instrumented internal pilot on a real high-cost migration, performance
investigation, security remediation, or architecture task, without modifying
the task's business source to fit Pong. Record both paths:

```text
Baseline A: Git branch/worktree + normal Agent session
Candidate B: Pong durable handoff/recovery
```

Record wall-clock setup, recovery, context reconstruction, repeated work,
provider-switch time, manual steps, failed-route retention, and operator
confidence. Run at least two independent high-cost tasks before changing the
verdict. If measured added complexity is greater than measured recovery value,
choose `KILL`; if the benefit repeats for the target teams, consider a narrowly
scoped route decision.

## Current Route State

```text
M4-020: COMPLETE
M4-021: COMPLETE / PASS
Provider Gate: PASS
Final E2E: PASS
Portable Execution State: CONDITIONAL GO / NOT RATIFIED
Official Next Slice: NONE
Roadmap: NEEDS_RECONCILIATION
Protocol v1.0: FROZEN
Production implementation: STOPPED
```
