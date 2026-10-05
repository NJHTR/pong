# Pong Real-World Scenario Validation

**Date:** 2026-10-05
**Baseline:** `f5a343b` (`dev`)
**Status:** `RESEARCH ONLY / NO IMPLEMENTATION AUTHORIZED`

## 1. Research Question

Would a real developer receive material value from Pong's provider-neutral,
forkable, portable, and recoverable Agent Execution State beyond the existing
combination of Git, worktrees, CI, and coding-agent sessions?

This document is a scenario analysis, not a user interview study or a product
ratification. It uses the five required task classes and the Core behavior
already demonstrated by the repository. It must not be read as evidence that a
new Slice is approved.

## 2. Target Users

The plausible target is narrow:

> Teams running long, high-cost engineering investigations or migrations where
> a task may outlive one provider session and a failed route must remain
> recoverable and explainable.

Examples include large dependency migrations, production performance work,
security remediation, and architecture investigations with multiple viable
solutions. The target is not every software developer. For ordinary edits,
Git and one Agent are cheaper and simpler.

The target user has all of these conditions:

- failure or rework is materially expensive;
- more than one technically credible route exists;
- work lasts long enough to cross process, session, or provider boundaries;
- preserving the failed route has decision value;
- a human can evaluate and select results.

## 3. Scenario A - Ordinary Small Change

**User:** individual developer
**Task:** add an API parameter, fix a small bug, or add one field
**Why difficult:** low uncertainty, short execution, easy rollback

| Dimension | Git + Agent | Current Pong | Pong + Forkable State |
| --- | --- | --- | --- |
| Setup cost | Minimal | Repository/bootstrap concepts | Higher |
| Operational complexity | Low | Medium | High |
| Time cost | Minutes | Extra coordination | Extra coordination |
| Failure recovery | Re-run or revert | Durable but unnecessary | Durable but unnecessary |
| Parallel exploration | Usually unnecessary | Available by composition | Available |
| Provider switching | New session plus instructions | Durable handoff possible | Durable handoff possible |
| State portability | Files and commits | Core records plus files | Same, with forks |
| Evidence preservation | Commit/PR/CI | Durable records | Durable records |
| Rollback | Git revert/reset | Version/checkpoint plus Git | Same |
| Human control | High | High | High |

**Result:** Git + Agent is sufficient. Pong adds ceremony without solving a
real pain. This scenario is a clear non-target.

## 4. Scenario B - Large Refactor

**User:** migration or platform team
**Task:** Spring Boot 2 to 3, monolith modularization, or a large data-access
refactor
**Why difficult:** broad change surface, hidden coupling, long test cycles,
and credible incremental and one-shot routes

Git branches and worktrees already isolate routes. CI can validate each route,
and a PR preserves the selected result. Pong adds durable Execution ownership,
Checkpoint recovery, and provider-neutral continuation when a route spans
multiple sessions or providers.

**Assessment:**

- Fork can reduce accidental interference, but Git worktrees already solve the
  filesystem problem.
- Retaining a failed route is useful when its evidence prevents repeating a
  costly migration attempt.
- Pong is valuable only when the migration is long-running and handoff or
  recovery is common; otherwise its setup cost dominates.

**Value:** Medium, conditional on task duration and failure cost.

| Dimension | Option A: Git + Agent | Option B: Current Pong | Option C: Pong + Forkable State |
| --- | --- | --- | --- |
| Setup cost | Low | Medium | High |
| Operational complexity | Low | Medium | High |
| Time cost | Low for one route | Medium | High before reuse benefits |
| Failure recovery | Re-run, revert, or switch branch | Durable checkpoint and execution recovery | Independent route recovery |
| Parallel exploration | Worktrees and multiple agents | Composable workspaces | Explicit independent routes |
| Provider switching | Manual handoff note | Durable handoff | Durable handoff per route |
| State portability | Files, commits, notes | Core facts and references | Fork facts plus references |
| Evidence preservation | CI, PR, ADR | Durable execution facts | Route-specific facts and checkpoints |
| Rollback | Git revert/reset | Version/checkpoint plus Git | Retain and select a route |
| Human control | High | High | High; no automatic winner |

## 5. Scenario C - Performance Optimization / Experiment

**User:** performance engineer
**Task:** compare indexing, caching, thread-pool, or serialization strategies
**Why difficult:** several routes may be valid, benchmark results are noisy,
and a failed hypothesis can still be useful evidence

The natural shape is:

```text
Base state C
  +-- Route A
  +-- Route B
  +-- Route C
```

Git worktrees plus CI/benchmark jobs already provide parallel code and result
isolation. Pong can additionally retain which Execution produced which Version,
Checkpoint, failure, and recovery history after a process restart.

**Assessment:** parallel work is useful, but the benchmark system remains the
source of truth for measurements. Pong does not replace it. Cross-provider
continuation is rarely required for a short benchmark run.

**Value:** Medium for long experiments; Low for ordinary benchmark branches.

| Dimension | Option A: Git + Agent | Option B: Current Pong | Option C: Pong + Forkable State |
| --- | --- | --- | --- |
| Setup cost | Low | Medium | High |
| Operational complexity | Low | Medium | High |
| Time cost | Parallel setup is familiar | Durable setup overhead | Parallel setup plus route bookkeeping |
| Failure recovery | Re-run benchmark or revert | Recover execution and workspace | Recover one route without affecting siblings |
| Parallel exploration | Worktrees and CI jobs | Multiple workspaces by composition | Independent route set from one checkpoint |
| Provider switching | Manual notes | Linear durable handoff | Portable handoff per experiment |
| State portability | Branch and artifact references | Core state references | Fork/checkpoint references |
| Evidence preservation | CI benchmark artifacts | Execution and version facts | Route facts plus comparison inputs |
| Rollback | Git revert/reset | Version/checkpoint recovery | Keep failed hypotheses immutable |
| Human control | High | High | High; human evaluates measurements |

## 6. Scenario D - Architecture Comparison

**User:** staff engineer or architecture group
**Task:** Kafka vs RabbitMQ, Redis Streams vs a queue, synchronous vs
asynchronous, or modular monolith vs services
**Why difficult:** implementation cost, operational risk, performance, and
team impact must be compared across different solutions

Two Git branches, two Agents, CI, benchmark results, ADRs, and a human review
already cover most of this workflow. Pong can preserve durable execution and
workspace provenance, but it cannot decide architectural fitness and has no
dedicated comparison or promotion semantic.

**Assessment:** Git + CI + two Agents is usually enough. Pong's additional
records help only when the investigation is long-running, interrupted, or
handed between providers.

**Value:** Low to Medium. The ordinary case is commoditized.

| Dimension | Option A: Git + Agent | Option B: Current Pong | Option C: Pong + Forkable State |
| --- | --- | --- | --- |
| Setup cost | Low | Medium | High |
| Operational complexity | Low | Medium | High |
| Time cost | Low for a short spike | Medium | High unless investigation is long-running |
| Failure recovery | Re-run branch/CI | Durable execution recovery | Independent route recovery |
| Parallel exploration | Two branches and CI | Multiple workspaces by composition | Durable architecture alternatives |
| Provider switching | Manual handoff | Linear handoff | Cross-provider route continuation |
| State portability | Commits, ADRs, artifacts | Core references | Route and checkpoint references |
| Evidence preservation | PR/ADR/CI | Execution facts | Facts tied to each route |
| Rollback | Git revert/merge choice | Version/checkpoint plus Git | Preserve selected and unselected routes |
| Human control | High | High | High; no automatic architectural decision |

## 7. Scenario E - Cross-Provider Long Task

**User:** AI-heavy engineering team with provider churn or strict continuity
requirements
**Task:** Codex starts a migration or investigation, reaches a durable
checkpoint, and Claude or another provider continues it after a crash, quota
limit, policy change, or session boundary
**Why difficult:** chat/session context is provider-specific, a new Agent must
not repeat completed or disproven work, and the task may outlive one process

The durable factual state is:

```text
Task scope
Workspace and source Version
Checkpoint reference
Execution lineage and outcome
Handoff relation
Artifact/evidence references
Continuation intent
```

M4-021 proves the linear Codex-to-Claude path with user-controlled independent
sessions, completion, fresh-process inspection, Repository reopen, and cold
reopen. It does not prove parallel cross-provider forks, automatic comparison,
or promotion.

**Assessment:** this is the strongest scenario. Git preserves files and
commits, and a human can write a handoff note, but those steps do not by
themselves preserve Core ownership, lease/revision state, checkpoint lineage,
or a durable completion/failure record. The value is real when reconstruction
would cost hours or risk repeating an expensive route.

**Value:** High for a narrow population; frequency is not yet proven.

| Dimension | Option A: Git + Agent | Option B: Current Pong | Option C: Pong + Forkable State |
| --- | --- | --- | --- |
| Setup cost | Low initially | Medium | High initially |
| Operational complexity | Manual handoff and recovery | Durable Core operations | Durable operations for sibling routes |
| Time cost | Reconstruct context after failure | Lower resume cost | Lower resume cost plus parallel route cost |
| Failure recovery | New session and handoff note | Checkpoint, Execution, and cold reopen | Isolated sibling recovery |
| Parallel exploration | Provider-specific worktrees/tasks | Composable independent workspaces | Explicit forked executions |
| Provider switching | Context must be restated | Provider-neutral factual handoff | Provider-neutral factual handoff per fork |
| State portability | Files plus human notes | Core-owned references | Portable route state from a checkpoint |
| Evidence preservation | Commits/notes/PRs | Durable operations and outcomes | Independent evidence per route |
| Rollback | Git revert/reset | Checkpoint/version recovery | Retain failed and selected routes |
| Human control | High | High | High; user controls provider launch and selection |

## 8. Alternative Solutions

| Alternative | What it already solves | Remaining gap relevant to Pong |
| --- | --- | --- |
| Git branch | Content history, merge, rollback | No Agent lifecycle or provider handoff |
| Git worktree | Isolated filesystem trees | No durable execution/checkpoint semantics |
| CI/test jobs | Repeatable validation and artifacts | Does not own Agent continuation |
| Cursor parallel agents | Product-scoped parallel work | Provider/product-specific state |
| Codex parallel agents | Long-running or parallel tasks | No shared Pong state contract |
| Claude Code sessions | Session continuity and tools | Provider-specific context |
| Factory/Missions | Mission/task orchestration | Not a provider-neutral Pong state |
| Microsoft Agent Framework | Workflow and host integration | Application must define code-state semantics |
| Temporal | Durable workflow execution | Not code workspace/version provenance |
| Pong | Durable Core facts across Agent, Execution, Workspace, Version, Checkpoint, Handoff | More setup; no named fork/compare/promote contract |

## 9. Capability Comparison

| Capability | Existing solution | Pong advantage | Pong disadvantage |
| --- | --- | --- | --- |
| Code isolation | Git branch/worktree | None for basic isolation | Adds concepts already solved by Git |
| Agent execution | Provider CLI/product | Provider-neutral durable facts | Does not launch providers |
| Checkpoint | Commits, task snapshots, provider sessions | Explicit immutable recovery reference | Requires Pong metadata and operations |
| Recovery | Git revert, provider retry, CI rerun | Execution, lease, Version, Checkpoint, and cold-reopen state | More failure modes and operational surface |
| Cross-provider continuation | Manual handoff or product-specific state | Durable factual handoff independent of provider | Linear path proven; parallel path unproven |
| Execution fork | Branch/worktree plus multiple agents | Can compose independent durable Executions | No named fork API or sibling relation |
| Execution provenance | Commit/PR/issue notes | Structured ownership and outcome references | Not automatically a decision record |
| Promote | Merge/cherry-pick/PR | Could retain selected and unselected durable state | No dedicated Promote semantic today |

Capabilities marked as ordinary code isolation, merge, and simple recovery are
`COMMODITIZED`; Pong must not sell them as unique.

## 10. User Pain and the Seven Value Candidates

| Candidate value | Rating | Finding |
| --- | --- | --- |
| Portable Execution State | **High** for cross-provider long tasks; Medium overall | Avoids reconstructing factual state after provider/session change. |
| Execution Fork | **Medium** | Useful for risky alternatives, but Git worktrees already isolate code. |
| Failure Isolation | **Medium** | Important for expensive parallel work; ordinary branches already isolate files. |
| Durable Recovery | **High** for long-running tasks; Low for small changes | Core persistence is meaningful when process/provider failure is costly. |
| Cross-provider continuation | **High** for targeted teams; Low frequency overall | M4-021 proves feasibility, not broad demand. |
| Promote | **Low to Medium** | Human selection plus PR/merge usually works; Pong lacks a proven semantic. |
| Provenance | **Medium** | Structured execution facts help, but commits, PRs, ADRs, and issues solve much of the need. |

## 11. What Pong Actually Adds

Pong adds one concrete capability beyond Git and provider sessions:

> A provider-neutral, durable record of which Agent Execution owns which
> Workspace and Version, what Checkpoint can be resumed, what happened after a
> failure, and how a fresh process can reopen that state without reconstructing
> it from chat.

It does **not** add a better Git branch, a provider launcher, an automatic
architectural judge, or a replacement for CI. It also does not yet provide a
named fork, comparison, or promotion contract.

## 12. Complexity Cost

Pong introduces costs that must be charged against the value:

- Repository/bootstrap metadata and new operational vocabulary;
- Agent, Execution, Workspace, Version, Checkpoint, lease, and Operation
  concepts;
- provider and transport boundary troubleshooting;
- evidence retention and unknown-outcome handling;
- a second system that users must understand beside Git and the provider CLI.

The complexity is justified only when manual reconstruction, lost failure
context, or provider lock-in costs more than this operational overhead. For a
small change or ordinary architecture branch, it is not justified.

## 13. Killer Scenario

The strongest candidate is a **long-running, high-risk production migration or
security remediation** where:

1. two or more routes are technically credible;
2. each route may require hours or days of Agent work and validation;
3. a provider can become unavailable, hit context limits, or be replaced;
4. a failed route must remain inspectable to prevent repeated work;
5. the team needs a fresh process to recover exact durable state;
6. a human, not an opaque automatic policy, selects the final result.

This scenario has high cost of a wrong decision and rework, but its market
frequency and willingness to adopt Pong are not yet measured. It is therefore
a validation target, not a ratified product requirement.

## 14. Scoring Matrix

Scores are 1-10. Higher is better for opportunity dimensions. For
`Competitive Threat` and `Implementation Cost`, lower is better.

| Direction | User Pain | Frequency | Willingness | Differentiation | Technical Fit | Competitive Threat | Implementation Cost | Defensibility |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A. Agent Memory | 6 | 8 | 6 | 2 | 3 | 9 | 7 | 2 |
| B. Work Graph / Provenance | 7 | 5 | 5 | 5 | 8 | 7 | 7 | 6 |
| C. Durable Execution | 8 | 4 | 6 | 6 | 9 | 6 | 6 | 7 |
| D. Multi-Agent Orchestration | 7 | 7 | 6 | 3 | 4 | 9 | 8 | 3 |
| E. Portable Execution State | 8 | 3 | 6 | 8 | 9 | 6 | 7 | 8 |
| F. Forkable Execution | 7 | 3 | 5 | 6 | 8 | 7 | 8 | 6 |
| G. Compare / Promote | 6 | 4 | 5 | 4 | 5 | 7 | 8 | 4 |

### Winner

**E. Portable Execution State**, but only as a narrow, evidence-led
possibility for high-cost cross-provider work.

### Runner-up

**C. Durable Execution**, as an enabling foundation rather than a standalone
user product.

### Kill or reject for current Pong positioning

- Agent Memory as a separate product direction;
- generic Multi-Agent Orchestration;
- automatic Compare/Promote policy;
- broad Forkable Execution as a feature promise without user evidence;
- Work Graph expansion into Core entities.

## 15. Product Positioning

Pong should be positioned, if it continues at all, as a narrow local durability
and handoff layer for expensive Agent work. It should not compete with Git,
CI, Cursor, Codex, Claude Code, Factory, or Temporal on their primary surfaces.

## 16. Verdict

```text
CONDITIONAL GO
```

The technical value is concrete in Scenario E and partially in long-running
Scenarios B and C. The ordinary scenarios are already well served by existing
tools, and no scenario evidence establishes high general-market frequency.
Therefore this is not a `GO`, and it is not yet a `KILL`: the right next step
is user validation, not implementation.

## 17. Why Pong Exists

> Pong exists to preserve and reopen provider-neutral, auditable Agent
> execution state when a high-cost engineering task must survive provider or
> process failure; it does not exist to replace Git or launch Agents.

If this sentence cannot be validated by real users with repeated costly
handoffs, the direction should be killed or repositioned.

## 18. Non-goals

This validation does not authorize:

- M4-022 or any new Slice;
- `fork_execution`, Compare, or Promote implementation;
- Work Graph, Exploration, Candidate, Evaluation, or Selection entities;
- Agent memory, vector storage, chat archives, or an orchestration engine;
- provider adapters or provider configuration changes;
- Protocol v1.0 changes;
- M4-021 rerun or provider startup;
- changes to `easyCode` or access to `D:\bs\seekwd`.

## 19. Next Validation

The next useful action is a small, instrumented user pilot with the target
teams, not production implementation. Ask participants to run one real
high-cost task using:

```text
Git + Agent
vs
Pong durable handoff/recovery
```

Measure reconstruction time after a provider/session failure, repeated work,
time to resume, evidence completeness, setup cost, and whether users would
keep the Pong metadata. A single successful demo is insufficient to change the
verdict to `GO`.

## Current Route State

```text
M4-020: COMPLETE
M4-021: COMPLETE / PASS
Provider Gate: PASS
Final E2E: PASS
Portable / Forkable Agent Execution State: CONDITIONAL GO / NOT RATIFIED
Official Next Slice: NONE
Roadmap: NEEDS_RECONCILIATION
Protocol v1.0: FROZEN
Production implementation: STOPPED
```
