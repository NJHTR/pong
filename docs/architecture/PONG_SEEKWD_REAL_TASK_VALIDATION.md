# Pong / seekwd Real-Task Validation

**Date:** 2026-10-05
**Validation status:** `BASELINE COMPLETE / PONG NOT EXECUTED`
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
Real Task Validation: BASELINE COMPLETE / PONG NOT EXECUTED
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

## 10. Experiment Isolation

The operator selected an explicit copy of the current dirty worktree as the
experiment input. The original seekwd repository remains untouched.

```text
Source: D:\bs\seekwd
Baseline Copy: D:\seekwd-pong-experiment\baseline
Pong Copy: D:\seekwd-pong-experiment\pong
```

The source and both copies were verified read-only after isolation:

```text
HEAD: c1f1a5b8d584a3d6e5f00e551acfe65db0edb309
Branch: dev
Tracked modified files: 16
Non-ignored untracked files: 0
Git-visible initial state: MATCH
Diff check: PASS
```

The copies were created from the Git checkout and then populated with the same
16 tracked modifications. The source working tree was not reset, restored,
checked out, stashed, committed, discarded, or staged. No provider, agent, or
Pong workflow was started.

The ignored/generated environment was intentionally not duplicated. The source
contains approximately 96,227 ignored/generated files (about 13.66 GiB,
including `target`, `references`, and `node_modules`). This is an explicit
experiment environment gap; dependency and build setup remain separate
preconditions for any later run.

```text
Baseline Ready: YES
Pong Ready: YES
A/B Started: NO
Reason: isolation only; execution deferred to a later operator step
Original Source Preserved: YES
```

## 11. Experiment Environment Parity

The ignored directories were inspected read-only in the source checkout. They
are not byte-for-byte experiment inputs; parity is based on reproducible
dependencies, identical Git-visible source state, identical task inputs, and
the same host toolchain.

| Ignored / Generated Directory | Purpose | Required? | Reproducible? | Material to Experiment? | Decision |
| --- | --- | --- | --- | --- | --- |
| `target` | Rust build artifacts and incremental compilation cache | Required only for build/test execution | Yes, generated by Cargo from the checkout and `Cargo.lock` | Only for preparation-time measurements, not task semantics | Do not copy; regenerate independently and record preparation time |
| `node_modules` | pnpm-installed frontend/tooling dependencies | Required for frontend typecheck/build/dev commands | Yes, with `pnpm@11.1.3` and `pnpm-lock.yaml` | Required if the task exercises Workbench/frontend paths, but reproducible | Do not copy; run the same frozen-lockfile install in both copies |
| `references` | Three nested external reference repositories (`Webintosh`, `Webintosh-Desktop`, `WebSwift`) | No runtime/build requirement found | Not needed for this task | No; no source, script, or runtime path reads this directory | Do not copy |

Observed source inventory:

```text
target: 23,979 files / 14,101,352,801 bytes
node_modules: 9,163 files / 138,645,464 bytes
references: 473 files / 402,369,714 bytes
```

The repository declares `pnpm@11.1.3`, uses `pnpm-lock.yaml` (lockfile version
9), and documents `pnpm install`, `pnpm typecheck`, and `pnpm build`. No
`rust-toolchain` or Node version file was found, so the later operator must
record `rustc`, `cargo`, `node`, and `pnpm` versions and use the same versions
for both copies. No preparation command was run in the original source or in
either experiment copy during this audit.

```text
Environment Parity: READY
Preparation: NOT EXECUTED
Preparation commands (per copy):
  pnpm install --frozen-lockfile
  cargo build --workspace
Task execution: NOT STARTED
Provider launch: NONE
```

## 12. Baseline Execution

Baseline A used only the isolated Git worktree and ordinary build/test
workflow. Pong Execution, Workspace, Checkpoint, Resume, and Handoff were not
used.

```text
Repository: D:\seekwd-pong-experiment\baseline
Task: Canvas Call + frozen Revision dependencies and legacy project-analysis workflow upgrade
Task start: 2026-10-05T23:41:48.3763058+08:00 +08:00
Initial HEAD: c1f1a5b8d584a3d6e5f00e551acfe65db0edb309
Initial branch: dev
Initial tracked modifications: 16
Initial non-ignored untracked files: 0
```

Toolchain:

```text
Rust: rustc 1.95.0 (59807616e 2026-04-14)
Cargo: 1.95.0 (f2d3ce0bd 2026-03-21)
Node: v22.17.0
pnpm: 11.1.3
```

Environment preparation was measured separately:

```text
pnpm install --frozen-lockfile: 3.595 s, exit 0
cargo build --release -p pong-host: 37.899 s, exit 0
```

The first workspace build attempt took 60.611 s and stopped because the
Tauri configuration required the release `pong-host.exe`. After that binary
was prepared, the next workspace attempt took 53.095 s and stopped because
the frontend `dist` directory did not yet exist. These are recorded as
preparation-order findings, not recovery measurements.

## 13. Baseline Interrupt Event

The first real task validation reached a meaningful interrupt point after the
existing Host/Core/schema/client changes were present and the Workbench had not
yet been fully validated.

```text
Interrupt: 2026-10-05T23:49:49.1367219+08:00 +08:00
HEAD: c1f1a5b8d584a3d6e5f00e551acfe65db0edb309
Working tree: 16 tracked modifications, 0 non-ignored untracked files
Completed: Canvas Call Host/Core/protocol/client changes and legacy workflow changes were present in the dirty task state.
Remaining: Workbench Canvas Call entrypoint and full build validation.
Failure: pnpm build reached apps/workbench and reported TS2367 at apps/workbench/src/main.tsx:916 and :926 because `canvas.call` was not represented by the Node Library/name mapping.
```

The provider/session was then treated as stopped. No Pong state was created,
and no provider switch was forced.

## 14. Baseline Git-only Recovery

Recovery used only the existing worktree, Git status/diff, the visible build
error, and the task's manual reconstruction. No Pong state or provider adapter
was used.

```text
Recovery start: 2026-10-05T23:51:51.8954801+08:00 +08:00
Recovery complete: 2026-10-06T00:00:55.0870002+08:00 +08:00
Measured recovery time: 9m 03.191s
Pre-recovery manual reconstruction: 2m 02.759s from interrupt to recovery start
Provider re-explanation: NOT REQUIRED
Provider switch: NOT REQUIRED
```

Recovery found and fixed one task-local omission in the Workbench: the Node
Library now exposes `Canvas Call`, and `nodeKindForName` maps it to
`canvas.call`. No Protocol or Pong code was changed.

The recovered task then passed:

```text
pnpm --filter @seekwd/workbench build: PASS
pnpm build: PASS
pnpm typecheck: PASS
cargo build --workspace: PASS
cargo test --workspace: PASS (45 pong-host + 2 provider + 5 workbench tests)
cargo fmt --all -- --check: PASS
pnpm check:requirements: PASS (19/365 IDs traced; remaining IDs unreviewed)
```

The first post-recovery full frontend build had a transient `ui-lab` process
exit `3221225477`; the immediate rerun passed. It was not converted into a
task failure and no source was discarded.

The recovered work was committed normally in the Baseline copy:

```text
Baseline checkpoint: c2ddf52757812d2df53ba8f3b59ef96035d7c83b
Commit: feat: add frozen canvas call workflow
```

## 15. Baseline Result

```text
BASELINE COMPLETE
Git-only recovery: COMPLETE
Lost work: NONE OBSERVED
Initial 16 dirty edits preserved: YES
Additional task work after recovery: Canvas Call Workbench entrypoint mapping
Pong: NOT EXECUTED
Pong Checkpoint/Resume/Handoff: NOT USED
Provider launch: NONE
Original seekwd: UNCHANGED
```

The measured result is that the normal Git workflow recovered the task without
code loss, but required manual reconstruction of the failed build context and
one targeted Workbench fix. Fine-grained time spent separately on code
inspection, error explanation, and reconstruction was not recorded and is not
estimated.

## 16. RECOVERY ENTRY AUDIT

This audit evaluates whether a fresh Agent can discover and consume an
existing Pong checkpoint without reading Rust internals or writing a custom
driver. It is a read-only capability audit; no provider was started and no
durable store was modified.

### Current Recovery Surface

The durable recovery surface exists in the Core/library. `Repository::open`
and `Repository::open_as_core_owner` reopen an existing `.pong` repository.
`AgentControl` exposes task, execution, workspace, operation, checkpoint,
handoff, resume, and composed `state` views. `MetadataStore` exposes direct
lookups for task, execution, workspace, checkpoint, snapshot, version, and
resume records. These APIs preserve the distinction between a workspace head
(`sha256:<digest>`) and a snapshot identity (`snp-<digest>`).

### Existing APIs

The relevant public calls are:

```text
Repository::open
Repository::open_as_core_owner
MetadataStore::task / list_tasks
MetadataStore::execution / list_executions
MetadataStore::workspace
MetadataStore::checkpoint / list_checkpoints
MetadataStore::snapshot_record
MetadataStore::version_record / list_versions
MetadataStore::list_resume_attempts
AgentControl::state
AgentControl::resume_from_checkpoint
AgentControl::resume_from_version
```

Checkpoint listing requires a task ID that the caller already knows. Exact
checkpoint lookup and the linked-record lookups are available once a caller
has a Rust `Repository` or `AgentControl` handle.

### Existing Examples

`examples/m4-021-operator.rs` demonstrates the M4-021 acceptance scenario,
including checkpoint, handoff, resume, fresh-process inspection, and cold
reopen. It is scenario-specific and hardcodes the task/workspace/execution
identifiers; it is not a general checkpoint discovery or recovery inspector.

### Existing CLI / Operator

Before this slice, `pong-agent-protocol --project-root PATH` and
`pong-agent-protocol --repository PATH --workspace-root PATH` consume JSONL
Protocol v1.0 requests and resolve bootstrap metadata. They do not expose a
human-readable `inspect checkpoint`, `list checkpoints`, or `resume checkpoint`
command. `README.md` describes Pong as a Rust library/core rather than a
public CLI, and `docs/protocol/CLI_DESIGN.md` remains a proposed contract.
The recovery entry was missing at the time of the audit.

After this slice, the `pong` binary provides the narrow provider-neutral
commands:

```text
pong recovery inspect --repository PATH --checkpoint CHECKPOINT_ID [--json]
pong recovery resume --repository PATH --checkpoint CHECKPOINT_ID [--agent-id ID] [--json]
```

The existing protocol binary remains unchanged and continues to be a transport
consumer rather than a recovery UI.

### Fresh-Agent Discoverability

Before implementation: **POOR.** A fresh Agent had to inspect source or write
a custom Rust driver. After implementation: **GOOD for the scoped recovery
flow.** `pong --help` and `pong recovery --help` discover `inspect` and
`resume`, and both commands accept only a repository path and checkpoint ID
(with optional stable JSON output).

### Checkpoint Inspectability

The supplied durable IDs were first checked by a temporary, read-only Rust
program outside the Pong repository, using only the existing public APIs. That
temporary driver was required before the recovery entry existed. The same
checkpoint is now inspectable through the production `pong` binary.

The full chain exists in:

```text
D:\pong\.seekwd-pong-experiment-20261006-b\.pong
```

Observed records:

```text
checkpoint: checkpoint:seekwd:pong-canvas-call
task:       task-seekwd-canvas-call-frozen-revision
execution:  execution-seekwd-pong-20261006
workspace:  workspace-seekwd-dirty
version:    ver-ef4e8beb27030d1730a0754f9f6585a58e210445c5b5c584b1588d1b1389d7ac
snapshot:   snp-15db43c213c3ed1679a5936e9ec3920d6902e631347a05915aa8e97f9766c410
root head:  sha256:15db43c213c3ed1679a5936e9ec3920d6902e631347a05915aa8e97f9766c410
```

The linked execution is durable and interrupted at the fresh-process
boundary; the linked workspace is `ready`; the version points to the supplied
snapshot; and one resume attempt references the checkpoint. The sibling
experiment store `D:\pong\.seekwd-pong-experiment-20261006\.pong` contains the
execution and workspace IDs but does not contain the supplied checkpoint or
snapshot, so it is not the complete recovery source.

### Resume Accessibility

Resume remains available through `AgentControl::resume_from_checkpoint` and
`AgentControl::resume_from_version`, and is now exposed by
`pong recovery resume`. The command resolves the checkpoint and linked Task,
uses the checkpoint actor by default (or an explicit `--agent-id`), and emits
the newly created durable resume record. It does not start a provider.

### Five-Minute Test

**YES for the scoped flow.** A fresh process can run `pong --help`,
`pong recovery --help`, `inspect`, and `resume` without reading Rust source or
writing a driver. The targeted CLI tests exercise this sequence and the
invalid-input paths.

### Exact Gap

```text
Before: RECOVERY ENTRY GAP
After:  RECOVERY ENTRY = READY (scoped inspect/resume entry)
```

Internal durability and recovery operations were already present. The missing
piece was the public/operator entry that discovers an existing checkpoint,
projects its linked durable context, and makes the existing resume operation
usable by a fresh Agent. That gap is now closed for the minimal provider-
neutral inspect/resume surface.

### Minimal Entry Implemented

The smallest provider-neutral operator surface over the existing APIs is now
implemented:

```text
pong recovery inspect --repository PATH --checkpoint CHECKPOINT_ID
pong recovery resume  --repository PATH --checkpoint CHECKPOINT_ID
```

`inspect` reads and renders the checkpoint plus linked task, execution,
workspace, version, snapshot, head digest, and available resume records.
`resume` calls the existing checkpoint-resume operation and renders the
resulting durable resume record. This proposal does not require a new
Protocol version, provider adapter, scheduler, or Core durable entity; it is
an operator/entry layer over the existing public APIs. Exact command naming
and authorization remain future design work.

### Whether Implementation Is Justified

**YES, and implemented as a narrowly scoped recovery-entry slice.** The audit
found a real operator usability failure, not a schema or durability failure.
The CLI, tests, and discovery documentation now address it without changing
Protocol v1.0 or Core durable semantics.

### Product Implication

Pong now has a product-usable, narrowly scoped fresh-Agent recovery path for
checkpoint inspection and resume. It is not an Agent Manager, provider
orchestrator, or general CLI suite. Provider launch remains user-controlled;
Protocol v1.0 and Core durable semantics remain unchanged.

### Recovery Entry Verification

```text
Production command: pong recovery inspect/resume
Targeted tests: cargo test --locked --test recovery_entry
Help discovery: PASS
Valid inspect: PASS
Valid resume: PASS (isolated test fixture)
Missing checkpoint: PASS (non-zero with NOT_FOUND)
Invalid repository: PASS (non-zero with NOT_FOUND)
Real experiment store mutation: NO
Provider launch: USER CONTROLLED / NONE
```
