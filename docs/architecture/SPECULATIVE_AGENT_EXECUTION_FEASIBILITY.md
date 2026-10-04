# Speculative Agent Execution Feasibility Study

**Date:** 2026-10-05
**Baseline:** `4ae4a7a` (`dev`)
**Status:** `ARCHITECTURE STUDY / NOT RATIFIED / NOT IMPLEMENTATION`
**Working name:** Speculative Agent Execution

This study asks whether Pong should support multiple independent Agent
Executions forked from one durable Checkpoint, with later comparison and
promotion. It intentionally challenges the idea. The result is not an M4-022
decision and does not authorize production code, Protocol changes, or a new
Slice.

## 1. Executive Summary

The proposed capability is not automatically valuable. Git branches and
worktrees, modern coding-agent products, workflow frameworks, and durable
execution systems already cover substantial portions of parallel work,
isolation, retry, and continuation.

Pong is not justified in rebuilding those surfaces.

However, a Pong execution fork can carry a combination that the comparison
systems do not present as one provider-neutral contract:

```text
immutable Version/Snapshot source
+ durable Checkpoint and recovery point
+ independent Execution ownership and lifecycle
+ independent Workspace lease/revision
+ provider-neutral Agent identity
+ Handoff and cross-provider continuation
+ durable result references
+ explicit comparison and promotion provenance
```

That combination is more than a Git branch, but the product value of parallel
forks and promotion is not yet proven. The correct verdict is:

```text
CONDITIONAL GO
```

The only credible future candidate is a very small **Portable / Forkable Agent
Execution State** experiment. It must remain a candidate while the official
route stays:

```text
Official Next Slice = NONE
Roadmap = NEEDS_RECONCILIATION
```

## 2. Research Question

Is there real user value in this workflow?

```text
Base Execution
      |
  Checkpoint C
      |
  +---+---+
  |       |
 E_A     E_B
  |       |
Result A Result B
      \   /
       Compare
          |
       Promote
```

The study tests whether this is a durable, provider-neutral work-state
capability or merely:

```text
Git branch + Git worktree + Agent runner
```

It also tests whether M4-021's linear Codex -> Checkpoint -> Claude flow
naturally extends to parallel execution. M4-021 proves handoff and resume; it
does not prove parallel fork, comparison, or promotion.

## 3. Market Comparison

The matrix uses conservative public capability categories. A `PARTIAL` or
`NOT EVIDENCED` result means the public product surface does not establish the
full provider-neutral semantic claimed here; it is not a claim that the
product cannot perform a narrower workflow.

| Capability | Git | Cursor | Codex | Claude Code | Copilot | Factory | Microsoft Agent Framework | Temporal | Pong |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Checkpoint | Content refs only | Product/runtime checkpoints documented | Long-running task state | Session/context continuity | Session/task persistence | Mission/task state | Workflow checkpoint patterns | Durable workflow state | **PASS Core** |
| Parallel execution | Branch/worktree primitives | Background/parallel agents | Parallel/long-running agents | Subagents/parallel workflows vary by setup | Coding-agent tasks | Parallel Droids/missions | Workflow orchestration | Concurrent activities/workflows | **COMPOSABLE, no fork command** |
| Branch/worktree isolation | **PASS** | **PASS** | Worktree/task integration | Workspace/process dependent | Cloud workspace/task isolation | Mission/workspace isolation | Host/application concern | Not code-worktree semantics | **PASS Workspace/version** |
| Recovery | Git object recovery | Agent/task recovery | Long-running task continuation | Session continuation | Task/session continuation | Mission continuation | Durable workflow recovery | **PASS** | **PASS Core** |
| Handoff | No Agent handoff semantics | Agent product workflow | Agent product workflow | Provider-specific continuation | Product workflow | Mission handoff | Message/workflow routing | Workflow signal/state | **PASS provider-neutral** |
| Cross-agent continuation | No | Product-scoped | Product-scoped | Product-scoped | Product-scoped | Product-scoped | Framework-defined | Workflow-defined | **PASS M4-021** |
| Fork from execution state | Branch from content | Agent task/worktree fork | Task/worktree fork | Session/task continuation | Task/workspace fork | Mission fork patterns | Workflow branch patterns | Workflow replay/continue | **Composes, not named** |
| Compare outcomes | Diff/log | Product/task views | Task results | Session results | Task results | Mission outcomes | Application-defined | Workflow result | **No dedicated semantic** |
| Promote one result | Merge/cherry-pick | Agent result selection | Human/project adoption | Human/project adoption | Human/project adoption | Mission output adoption | Application-defined | Application-defined | **No dedicated semantic** |
| Cross-provider portable state | **NO** | **NO** | **NO** | **NO** | **NO** | **NOT EVIDENCED** | Provider/application mapping | Worker-agnostic workflow state, not code state | **Linear path PASS; parallel path unproven** |

Public references used for the boundary study include [Git worktree
documentation](https://git-scm.com/docs/git-worktree), [Cursor Background
Agents](https://docs.cursor.com/en/background-agent), [Claude Code
documentation](https://docs.anthropic.com/en/docs/claude-code/overview),
[GitHub Copilot coding agent](https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent),
[Microsoft Agent Framework](https://learn.microsoft.com/en-us/agent-framework/overview),
and [Temporal durable execution](https://docs.temporal.io/evaluate/understanding-temporal).
Codex and Factory capabilities are treated conservatively as provider/task
or mission orchestration, not as proof of a shared portable Pong state.

## 4. Git vs Execution Fork

### Git branch

Git preserves content history and named references. A branch does not own an
Agent lifecycle, provider identity, lease, checkpoint reason, handoff, or
durable execution outcome.

### Git worktree

A worktree provides another filesystem view for a Git checkout. It does not
define durable Agent Execution state, cross-provider resume, Operation
ownership, or evidence-backed promotion.

### Pong execution fork

A candidate fork would be:

```text
Checkpoint
+ source Version/Snapshot
+ independent Workspace
+ independent Execution and Operation state
+ independent lease/revision
+ new provider-neutral Agent continuation
+ independent Checkpoint chain
+ result and evidence references
```

If a user only needs two code trees and later merges a patch, Git plus a
runner is sufficient and Pong should be killed for that use case. Pong is
justified only where recovery, ownership, handoff, evidence, and cross-agent
provenance are materially important.

## 5. Current Pong Capabilities

Already solved and therefore not new value:

- Repository and immutable CAS-backed Snapshot state;
- Workspace materialization, leases, revisions, and CAS checks;
- Version lineage and explicit Version head;
- Execution lifecycle, ownership, parent linkage, and outcome;
- Checkpoint creation and immutable source references;
- Resume into a new Execution;
- Handoff between Executions;
- fresh-process inspection and Repository cold reopen;
- provider-neutral logical Agent identity and transport/session separation;
- M4-021 real user-controlled Codex/Claude continuation.

Potentially new, but not implemented as one contract:

- a named `fork_execution` composition relation;
- parallel Execution result comparison;
- canonical promotion of one result;
- portable state packaging for a different provider;
- durable linkage from those facts to one research/work node.

## 6. Minimal Execution Fork Model

The smallest credible model is:

```text
Checkpoint C
  |
  +-- Execution E_A -> Workspace A -> Version A -> Checkpoints A*
  |
  +-- Execution E_B -> Workspace B -> Version B -> Checkpoints B*
```

### Shared by reference

- Repository and Core ownership boundary;
- source Version and Snapshot;
- source Checkpoint record;
- immutable CAS objects and read-only evidence artifacts;
- Task and project/environment scope, subject to existing validation.

### Independent

- Execution identity and lifecycle state;
- Operation ownership and retry identity;
- Workspace materialization and locator;
- Workspace lease, epoch, revision, and current head;
- current Version selection for each Workspace;
- subsequent Checkpoint chain;
- provider execution/session and Agent continuation;
- mutable output artifacts.

No Repository clone and no whole-workspace copy is required. Materialization
from an immutable source creates the independent writable state.

## 7. Portable Execution State

The meaningful portability target is not a provider transcript. It is a
provider-neutral durable state bundle:

```text
Task scope
Workspace/source Version reference
Checkpoint reference
Execution lineage and outcome
Handoff relation
required artifact/evidence references
explicit continuation intent
```

Provider A and Provider B remain external executors. Provider B does not
receive hidden reasoning or a copied session; it receives the durable factual
state and must satisfy the same Core ownership and lease rules.

### Is this a real need?

It is a real need when:

- a long-running task outlives one provider session;
- a provider is unavailable or unsuitable for the next action;
- a team wants independent implementation attempts from one known state;
- the next Agent must avoid repeating an already disproven path;
- recovery must be based on durable state rather than chat reconstruction.

It is not a need when the user only wants parallel code edits or a final Git
merge. Those are already well served by Git worktrees and agent products.

Therefore portability is valuable but conditional on evidence and task
continuity, not an architectural virtue by itself.

## 8. Promote Semantics

`Promote(B)` must not mean `git merge B` and must not delete A or C.

The promotion target is an explicit state result, normally:

```text
selected Version/Snapshot from E_B
  -> promotion decision with evidence and actor
  -> authorized materialization/publication into a chosen canonical Workspace
```

Promotion does not automatically:

- complete the Task;
- transfer or rewrite Execution state;
- merge unrelated Version histories;
- delete or abandon other Executions;
- make B globally best;
- revoke recovery from A or C.

After promotion, A and C remain inspectable and recoverable. A later fork may
start from any explicit retained Checkpoint or Version. The canonical state is
a selected Workspace/Version reference plus an append-only promotion record,
not a mutable global winner hidden inside Core.

## 9. Real-world Scenarios

### A. Large refactor

Two routes start from one Checkpoint: incremental refactor and one-shot
refactor. Git branches already provide code isolation. Pong adds value only if
the team needs durable Execution ownership, failure/recovery history, evidence
of validation, and a provider-neutral continuation after one route fails.

**Assessment:** Git is sufficient for ordinary code branching; Pong is useful
for long-running, multi-agent, evidence-heavy refactors.

### B. Performance optimization

DB indexing, Redis, and thread-pool routes each run from C. Their benchmark
artifacts, Execution outcomes, cost, and failed hypotheses can be compared.

**Assessment:** Git plus CI can compare code and benchmark outputs. Pong's
additional value is durable route provenance and recovery, not the benchmark
runner itself.

### C. Cross-provider execution

Codex and Claude fork from the same Checkpoint and work in independent
Workspaces.

**Assessment:** M4-021 proves the linear handoff half. Parallel portable state
is plausible and fits existing Core semantics, but remains unproven and is the
most important validation target.

### D. Agent crash

E_A reaches C, crashes, and a new Agent resumes from C.

**Assessment:** Pong already solves this through Checkpoint, Resume, durable
Execution lineage, and cold reopen. It is not new fork value.

### E. Failed route and re-fork

Route A fails after C4. Route B starts from C4 while A remains immutable
history.

**Assessment:** Existing Version/Checkpoint/Workspace/Resume composition can
support the state transition. The new value is explicit route/result
provenance and comparison, not a second recovery engine.

## 10. Provider Neutrality

Pong must treat Codex, Claude, Copilot, Cursor, Factory, or another runtime as
an external executor. Core knows:

```text
Agent identity
Execution
Workspace
Version
Checkpoint
Handoff
Artifacts/evidence references
```

Core does not know how to invoke a provider, select a model, discover a CLI, or
translate provider-specific session state. Provider portability means
portable factual state and explicit continuation, not portable hidden context.

## 11. Protocol Impact

Protocol v1.0 remains:

```text
FROZEN / UNCHANGED
```

The minimal fork can be composed internally from existing Repository,
Workspace, Version, Snapshot, Execution, Checkpoint, Resume, Handoff, Lease,
and Revision/CAS APIs. There is no basis in this study for adding
`fork_execution`, `compare_execution`, or `promote_execution` to v1.0.

If an external operator cannot express the required workflow through current
commands and queries, record:

```text
FUTURE CONTRACT GAP
```

Do not hide fields in v1.0 or introduce a provider-specific adapter.

## 12. Technical Feasibility

### Feasible by composition

1. Read a source Checkpoint and its Version.
2. Materialize the source into Workspace A and Workspace B.
3. Create independent Executions bound to the respective Workspaces.
4. Acquire independent leases and run each provider externally.
5. Publish independent Versions and Checkpoints.
6. Inspect both after a fresh Protocol process and Repository reopen.

The current Core already supplies the integrity and recovery primitives for
these steps.

### Not yet expressed as a supported capability

- one named atomic `fork_execution` operation;
- durable relation grouping the sibling Executions;
- comparison record and criteria;
- promotion decision and canonical target policy;
- portable artifact/evidence package contract;
- external Protocol surface for fork/compare/promote.

These are capability gaps in composition semantics, not proof of a Core schema
gap. No implementation should start until a small experiment establishes user
value and exact failure/recovery boundaries.

## 13. User Value

The strongest value is avoiding repeated work and preserving decision quality:

```text
This path was tried.
It failed for this evidenced reason.
This result came from this Agent and Version.
You can safely resume from this Checkpoint.
Route B was promoted because these facts were stronger.
```

The value is high for long-running investigations, migrations, performance
work, and provider churn. It is low for ordinary single-agent edits where Git
and CI already provide sufficient history.

## 14. Differentiation

Pong is potentially distinct from:

- Git: adds execution ownership, recovery, handoff, and causal provenance;
- Cursor/Codex/Factory: is provider-neutral and durable beyond one product;
- Claude/Copilot memory: stores factual work state, not preferences or chats;
- Temporal: links durable workflow state to code Versions, Workspaces,
  Checkpoints, artifacts, and Agent handoff rather than only workflow tasks.

The risk is substantial: without evidence, route semantics, and promotion
provenance, the result collapses into Git worktrees plus an Agent runner. The
future experiment must therefore measure repeated-work avoidance and safe
cross-provider continuation, not just successful parallel execution.

## 15. Risks

- Rebuilding features already supplied by coding-agent products.
- Turning research semantics into unstable Pong Core entities.
- Confusing Version lineage with route lineage.
- Treating provider session state as portable when it is not.
- Automatic scoring or promotion becoming an opaque policy engine.
- Complex reconciliation between composition records and Core unknown outcomes.
- Evidence storage leaking credentials, transcripts, or hidden reasoning.
- A new Protocol surface before external demand is demonstrated.
- Measuring a green test instead of a real user problem.

## 16. Scoring Matrix

Scores are heuristic, from 1 to 10. Higher is better for every column except
`Risk of becoming Git + Agent Runner`, where lower is better. Adjusted score is
the sum of the first six dimensions minus the risk score.

| Direction | User Value | Market Demand | Differentiation | Technical Difficulty | Fit with Pong | Defensibility | Risk | Adjusted |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| A. Work Graph / Provenance | 9 | 8 | 8 | 7 | 8 | 7 | 3 | 44 |
| B. Speculative Execution | 8 | 7 | 6 | 6 | 9 | 6 | 6 | 36 |
| C. Portable Execution State | 8 | 6 | 8 | 8 | 9 | 8 | 4 | 43 |
| D. Agent Orchestration | 8 | 9 | 3 | 8 | 4 | 3 | 9 | 26 |
| E. Memory | 7 | 9 | 2 | 4 | 3 | 2 | 9 | 18 |
| F. Durable Execution | 8 | 8 | 4 | 7 | 10 | 6 | 5 | 38 |

The scores do not ratify a roadmap. They show why the strongest direction is
the combination of A and C, with B as a narrow validation mechanism, while D
and E should be rejected as product positioning.

## 17. Verdict

```text
CONDITIONAL GO
```

### Why not KILL?

Execution fork is not identical to Git branch/worktree when it includes
durable Execution ownership, Checkpoint lineage, lease/revision isolation,
provider-neutral continuation, evidence, and recoverable outcomes. Temporal
and coding-agent products cover adjacent pieces but do not establish this
exact code-state-plus-agent-provenance contract.

### Why not GO?

Pong already composes much of the mechanics, and the market already offers
parallel agents and task orchestration. No evidence yet proves that users need
a separate fork/compare/promote product rather than Git, CI, and an Agent
runner. Parallel portable state and promotion must be tested with a real
workflow before ratification.

## 18. Minimal Future Slice Candidate

Only as a proposal:

```text
Future Candidate: Portable / Forkable Agent Execution State
Status: CONDITIONAL / NOT RATIFIED
```

The smallest experiment would verify:

1. One Checkpoint produces two independent Executions.
2. Each Execution uses an independent Workspace and lease.
3. Both produce independent Version/Checkpoint chains.
4. A different provider resumes one state without provider session transfer.
5. Both results can be inspected after fresh-process and cold reopen.
6. A factual comparison records inputs and evidence.
7. One result can be promoted into an explicit canonical Workspace/Version.
8. The unselected Execution remains recoverable and immutable history remains.

The experiment must not add LLM Judge, automatic scoring, route selection,
Vector DB, Memory Store, Chat Archive, provider adapter, MCP, remote execution,
cluster, Kubernetes, or multi-region infrastructure.

## 19. Non-goals

This study does not authorize:

- M4-022;
- `fork_execution` implementation;
- Promote implementation;
- Work Graph or Exploration API;
- provider adapters;
- Memory or Vector DB;
- Protocol v1.0 changes;
- M4-021 rerun;
- E3 or E1/E2 expansion.

## 20. Open Questions

- Which real user workflow cannot be completed economically with Git, CI, and
  an existing Agent runner?
- Does cross-provider state portability reduce repeated work enough to justify
  a new composition contract?
- Is promotion best represented as a Core Version-head operation or an
  upper-layer decision followed by explicit materialization?
- How should unknown provider outcomes affect sibling fork status?
- Which artifacts can be referenced without introducing a new artifact store?
- Can the first experiment remain entirely outside Protocol v1.0?
- What user evidence would change this verdict from CONDITIONAL GO to GO or
  KILL?

## Current Route State

```text
M4-020: COMPLETE
M4-021: COMPLETE / PASS
Provider Gate: PASS
Final E2E: PASS
Official Next Slice: NONE
Roadmap: NEEDS_RECONCILIATION
Protocol v1.0: FROZEN
Production implementation: STOPPED
```

This is a feasibility study only. It does not ratify the future candidate.
