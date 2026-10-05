# Pong / seekwd Real-Task Validation

**Date:** 2026-10-05
**Validation status:** `TASK BASELINE ESTABLISHED / PAUSED`
**Pong baseline:** `251d4d4` (`dev`)
**seekwd baseline:** `c1f1a5b8d584a3d6e5f00e551acfe65db0edb309` (`dev`)

This record establishes a real-task baseline only. The controlled Git-only
workflow and Pong workflow have not been executed. No value, timing, recovery,
or provider-switch claim is made yet.

## 1. seekwd Git Baseline

The repository was inspected read-only at:

```text
D:\bs\seekwd
```

Observed state:

```text
Branch: dev
HEAD: c1f1a5b8d584a3d6e5f00e551acfe65db0edb309
Upstream: origin/dev
origin/dev: c1f1a5b8d584a3d6e5f00e551acfe65db0edb309
Working tree: DIRTY
Tracked modified files: 16
Untracked files: none reported
```

The existing modifications were not created, reset, staged, stashed, or
otherwise changed by this validation. The dirty files include business code,
Rust Host/Core, protocol schema/generated types, client code, and design
contracts. Because the worktree is user-owned and already in progress, no
experiment may overwrite or isolate it without an explicit operator-selected
copy/worktree.

The current HEAD commit is:

```text
c1f1a5b fix(workbench): upgrade legacy project analysis workflows
```

The preceding commits show the task's active context:

```text
9dae629 feat(runtime): build bounded project context
74c883f feat(workbench): compose bounded project context
69302cd test(runtime): cover project context pipeline
c1f1a5b fix(workbench): upgrade legacy project analysis workflows
```

## 2. Selected Real Task

```text
Add executable Canvas Call composition with frozen target revisions and use it
to upgrade legacy project-analysis workflows.
```

The current uncommitted change set adds or changes:

- `canvas.call` node ports and Workbench node configuration;
- `FrozenCanvasDependency` and dependency data in `RunPlanSnapshot`;
- Host-side graph freezing, digest validation, entrypoint validation, cycle and
  depth checks, same-workspace enforcement, and `call_and_wait` execution;
- protocol schema and generated TypeScript types;
- client port mapping;
- entrypoint, Canvas, roadmap, decision, and evidence-ledger wording;
- the `async-recursion` Host dependency.

The observed diff is approximately 480 added/changed lines across 16 tracked
files. This is a multi-module task with a non-trivial contract surface, not a
toy example.

## 3. Why This Task Is Actually High-Cost

The task has real complexity because correctness spans:

1. immutable Revision selection rather than live Draft execution;
2. graph digest and entrypoint consistency;
3. nested Canvas Call dependency freezing;
4. cycle and maximum-depth rejection;
5. same-Workspace restrictions;
6. runtime input/output propagation and `call_and_wait` behavior;
7. compatibility between Rust, JSON schema, generated TypeScript, client, and
   Workbench behavior;
8. legacy project-analysis workflow migration without breaking existing runs;
9. recovery and repeatability after Host/process interruption.

The task can plausibly span hours because a defect may only appear at the
boundary between saved revisions, nested execution, generated types, and the
frontend's legacy graph migration. The current state also has an explicit
interruption risk: it is an unfinished dirty worktree with changes spread
across the application and contracts.

## 4. Current Objective and Acceptance Shape

The objective inferred from the real diff and current design contracts is:

```text
Allow a parent Canvas to call a saved Revision of another Canvas in the same
Workspace, freeze that dependency into the parent Run plan, execute it with
call_and_wait semantics, and preserve deterministic validation boundaries.
```

The acceptance shape to confirm before any experiment is:

- target Canvas and positive saved Revision are required;
- the target Revision includes a valid default entrypoint when required;
- graph JSON digest matches the recorded content digest;
- frozen entrypoint and compiled node order remain consistent;
- cross-Workspace calls are rejected in Local Restricted mode;
- recursive cycles and depth above eight are rejected;
- called Canvas execution returns its Result and completion flow;
- a paused child is rejected for synchronous `call_and_wait`;
- legacy project-analysis workflows still build and execute;
- generated schema/client/Workbench types remain aligned;
- existing run-plan and Host behavior remains compatible.

No acceptance command was run in this baseline phase. These criteria are a
task map, not a claim that the dirty change set passes them.

## 5. Existing Context

The repository's README states that the Workbench currently provides an MVP
Canvas/Run flow and that Host Run behavior remains a simulated completion path
without a general Worker or unrestricted execution environment. The current
task therefore extends a real but still internal workflow; it is not a claim of
production-ready public execution.

The design documents explicitly define `canvas.call`, immutable Revisions,
entrypoint semantics, cross-Canvas delivery, and recovery requirements. The
current diff is attempting to align implementation with those contracts rather
than inventing an unrelated feature.

## 6. Experiment Boundary

The required value experiment is **not started**.

### Baseline A: ordinary Git + Agent

Before Pong is introduced, an operator would use a separately selected clean
worktree or copy of this exact seekwd state, then use the normal provider,
commits/notes, tests, and a human handoff. The dirty user worktree itself must
not be used as an experimental scratch area.

Required measurements remain:

```text
start time: NOT RECORDED
context preparation: NOT RECORDED
first working checkpoint: NOT RECORDED
interrupt time: NOT RECORDED
manual reconstruction: NOT RECORDED
provider switch: NOT RECORDED
lost work: NOT RECORDED
completion: NOT RECORDED
```

### Candidate B: Pong

Only after Baseline A is complete and the operator explicitly selects an
isolated equivalent worktree may Pong be used for:

```text
Execution -> Workspace -> Version/Snapshot -> Checkpoint
-> real interruption -> Resume/Handoff -> fresh process -> cold reopen
```

Provider launch must remain user-controlled. No provider was launched in this
baseline phase.

## 7. Why Validation Is Paused

The current seekwd worktree is dirty and contains user-owned changes across
production code and contract documents. Running an A/B experiment directly in
it would make it impossible to attribute timing, recovery, or lost work to the
workflow rather than to the pre-existing state.

The next operator decision must therefore choose one of these safe inputs:

```text
1. a clean isolated worktree at c1f1a5b;
2. an explicit snapshot/copy of the current dirty worktree, preserving all
   changes and recording its exact content identity;
3. stop because no safe equivalent experiment input is available.
```

This document does not choose among them and does not perform the copy.

## 8. Current Value Status

```text
High-cost real task: AVAILABLE (baseline identified)
Reliable A/B baseline: NOT YET AVAILABLE
Real interruption: NOT EXECUTED
Recovery timing: NOT RECORDED
Provider handoff timing: NOT RECORDED
Lost work: NOT RECORDED
Pong Moment: NOT OBSERVED
Value verdict: NOT YET DETERMINED
```

The existing M4-021 calculator handoff is not reused as value evidence. It
only establishes that the Pong mechanism can support a provider-neutral durable
handoff; this seekwd task is the first candidate with enough cross-module
complexity to justify a real A/B attempt.

## 9. Route State

```text
M4-020: COMPLETE / PASS
M4-021: COMPLETE / PASS
Real Task Validation: BASELINE ESTABLISHED / PAUSED
Official Next Slice: NONE
Roadmap: NEEDS_RECONCILIATION
Protocol v1.0: FROZEN
Pong production changes: NO
seekwd changes by this validation: NO
Provider launch: NONE
```

No `GO`, `CONDITIONAL GO`, or `KILL` product verdict is assigned yet. The
experiment must stop here until a safe isolated input and an operator-owned
baseline run are available.
