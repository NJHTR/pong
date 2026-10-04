# Durable Agent Exploration Infrastructure Proposal

**Date:** 2026-10-05
**Baseline:** `be01ff4` (`dev`)
**Status:** `PROPOSED ARCHITECTURE / NOT RATIFIED / NOT IMPLEMENTATION`

This document proposes a durable exploration model after M4-021. It does not
create M4-022, authorize production changes, change Protocol v1.0, or promote
the historical E1/E2 implementation into a Core contract.

## 1. Problem Statement

Pong now has durable infrastructure for reliable agent work: Repository,
Workspace, Snapshot, Version, Execution, Checkpoint, Resume, Handoff,
Materialization, Rollback, Operation, leases, revision/CAS, and recovery. M4-020
proved the supported native runtime path across Windows, Ubuntu 24.04, and
macOS. M4-021 proved a user-controlled Codex-to-Claude continuation through
the existing JSON Lines transport, including fresh-process inspection and cold
reopen.

Those capabilities answer:

> How does an agent perform, publish, recover, and hand off reliable work?

They do not by themselves answer:

> How does a team preserve the durable history of investigating one problem
> through multiple alternatives?

The missing concern is exploration history: route alternatives, research
context, failed-path preservation, explicit fork provenance, cross-agent
research handoff, and auditable decisions. Chat transcripts are not a durable
exploration model. Provider session history is neither complete nor portable
enough to be the source of truth.

## 2. Design Goals

- Preserve alternative routes and failed work without rewriting Core history.
- Make every fork source explicit and inspectable.
- Reuse existing Version, Workspace, Execution, Checkpoint, Resume, Handoff,
  Materialization, Rollback, Operation, lease, and recovery semantics.
- Keep provider identity, runtime identity, transport identity, and exploration
  identity distinct.
- Record evidence and decisions without forcing a universal numeric score or
  an automatic winner.
- Permit independent composition/research tooling to compare routes while Pong
  remains a state and execution authority.
- Keep Protocol v1.0 frozen until a separately approved contract gap is proven.

## 3. Non-goals

This proposal does not authorize:

- a new Pong Core Exploration, Route, Candidate, Evaluation, or Selection API;
- Git branches, commits, merge, rebase, tracking, or repository replacement;
- automatic search, scheduling, ranking, or LLM judging;
- provider-specific Codex, Claude, or Cursor adapters;
- storage of provider raw chat, hidden reasoning, credentials, or session
  internals;
- E3 implementation or expansion of E1/E2;
- M4-022 ratification or any new official next Slice.

## 4. Terminology and Ownership

The proposal separates three layers:

1. **Pong Core** owns authoritative state, execution, filesystem materialization,
   leases, operations, recovery, and provider-neutral lifecycle records.
2. **Composition / Research Layer** owns the meaning of an exploration, its
   routes and nodes, hypotheses, evidence, comparisons, and decisions. It may
   persist references to Pong records and compose Core operations.
3. **External Provider** owns provider-specific process state, thread/session
   details, prompts, and raw output. It reports work through the provider-neutral
   boundary; it does not define Pong history.

An Exploration object can be durable without being a Pong Core entity. The
recommended first design is a durable upper-layer aggregate whose references
to Core records are validated, immutable where appropriate, and recoverable.

## 5. Concept Inventory

| Concept | Purpose | Owner | Durable requirement | Core status |
| --- | --- | --- | --- | --- |
| Exploration | Scope for alternatives around one Task | Composition/Research | Yes: stable identity, Task binding, purpose | Not a Core entity |
| Route | One explicit investigative path | Composition/Research | Yes: source anchor, status, node membership | Not a Core entity |
| Node | A named research state/event in a Route | Composition/Research | Yes: parent, state refs, outcome refs | Not Execution/Version |
| Fork relation | Explains why a new Route starts from a prior state | Composition/Research | Yes: base node/source and new route | Composes Core APIs |
| Hypothesis | Claim being investigated | Composition/Research | Yes when it affects route/decision history | Not Core |
| Action | Intended work performed at a node | Composition/Research + Core refs | Intent may be durable; actual run is Core Execution | No new Core action type |
| Observation | Reported result of an action | Composition/Research | Yes when used as evidence or decision input | Opaque reference/payload outside Core |
| Evidence reference | Link to artifact, test, observation, or report | Composition/Research | Yes, without hidden reasoning | Core stores only approved opaque refs |
| Decision | Human/orchestrator decision about a route or hypothesis | Composition/Research | Yes, actor, reason, evidence refs | Not automatic Core state |
| Execution | Concrete agent run and lifecycle | Pong Core | Yes, already durable | Existing Core entity |
| Workspace | Authorized writable materialization | Pong Core | Yes, already durable | Existing Core entity |
| Version/Snapshot | Immutable state and content lineage | Pong Core | Yes, already durable | Existing Core entities |
| Checkpoint | Recovery anchor | Pong Core | Yes, already durable | Existing Core entity |
| Resume | New execution from explicit source | Pong Core | Yes through Resume/Execution records | Existing Core semantics |
| Handoff | Context/ownership relation between executions | Pong Core | Yes, already durable | Existing Core semantics |
| Comparison | Structured view over route evidence/outcomes | Composition/Research | Persist result or reproducible input refs | Not Core |
| Selection | Scoped choice of a Candidate/Route | Composition/Research | Yes, append-only decision record | Not global Core winner |
| Promotion | Explicit adoption of a selected state | Composition/Research + explicit Core call | Decision is durable; materialization/publication remains Core | Not automatic |

`Node` is deliberately not an alias for Execution, Version, Snapshot, or
Checkpoint. It is a composition-layer relation that explains the research
meaning of a state and points to the Core records that made that state real.

## 6. Exploration Graph Model

The proposed graph is an upper-layer graph over two existing Core graphs:

```text
Exploration (one Task)
  |
  +-- Route A
  |     +-- Node A0 -- Execution(s) -- Checkpoint/Version refs
  |     +-- Node A1 -- Execution(s) -- Version V1
  |     +-- Node A2 -- failed Execution -- Checkpoint C1
  |
  +-- Route B (forked from Node A1 / V1 or C1)
  |     +-- Node B0 -- materialized Workspace B
  |     +-- Node B1 -- Execution(s) -- Version V3
  |
  +-- Route C ...
```

The graph is not a replacement for the Core graphs:

```text
Exploration graph: Exploration -> Route -> Node -> Core references
Execution graph:   Task -> Execution -> Operation/Event
Version graph:     Version -> parent Version -> Snapshot
Recovery graph:    Checkpoint -> Resume -> new Execution
```

An Exploration Root is the durable scope and initial source context. A Route
is an explicit alternative path in that scope. A Node is a named research
state or transition point, not a physical tree and not a process. A node may
reference a Version, Checkpoint, Execution, evidence set, or decision, but the
references do not change the authority of those Core records.

## 7. Node and Execution Relationship

A node may have zero or more executions over its lifetime. For example, a
provider failure can leave a node with an interrupted Execution and a durable
Checkpoint, after which a later Execution resumes from that checkpoint. The
node records the research state and relation; each Execution records one
concrete attempt.

Recommended node references:

- `node_id`, `route_id`, optional `parent_node_id`;
- source/base `version_id` or `checkpoint_id` when the node is created;
- zero or more explicit `execution_id` references;
- optional terminal/current `version_id` and evidence references;
- route-layer status and decision metadata.

The node must not own a Workspace. It references the Workspace used by an
Execution or materialization record. It must not duplicate Snapshot bytes or
Version metadata. Agent identity is attached through Core Execution records,
not redefined by the node.

## 8. Fork Semantics

`fork_exploration(base_node)` means:

```text
Base Node
   |
   +-- explicit source Version or Checkpoint
          |
          +-- new Route
                 |
                 +-- new Node
                        |
                        +-- new Execution in a separate Workspace
```

The fork operation is a composition workflow, not a new Core recovery
engine:

1. Validate that the base node, source Version/Checkpoint, Task, project, and
   environment are compatible.
2. Create a new Route with an immutable source anchor and a new Route identity.
3. Materialize or resume the source into a distinct writable Workspace.
4. Create a new Execution and attach it explicitly to the new Route/Node.
5. Publish later Versions with explicit parent/version semantics.
6. Persist the fork relation and evidence after the Core operations succeed.

The old Route, nodes, executions, checkpoints, versions, and snapshots remain
immutable history. The old Route may continue; a fork is not a transfer or
implicit abandonment. Shared immutable source objects may be reused. Writable
Workspace state, leases, revisions, current Version references, and new
Execution identity are independent. No repository copy or full database clone
is required.

## 9. Route Lifecycle and Failure Knowledge

Keep route status small and explicit:

```text
ACTIVE -> BLOCKED
ACTIVE -> FAILED
ACTIVE -> COMPLETED
ACTIVE -> ABANDONED
COMPLETED -> PROMOTED
```

`BLOCKED` means work cannot currently proceed and records an external or
policy reason. `FAILED` records a route-level conclusion after the route's
attempts are exhausted. `ABANDONED` is an explicit human/orchestrator choice.
`COMPLETED` means the route achieved its declared research stopping
condition. `PROMOTED` means a separate decision adopted its result; it does
not rewrite the route or make it globally best.

Execution failure is not automatically route failure. A provider failure,
interruption, or unknown outcome remains the Core Execution/Operation result.
The Composition layer may later record a route decision that the hypothesis
was rejected. That distinction preserves failed-path knowledge without making
Core infer research meaning from provider callbacks.

Example durable research record:

```text
Hypothesis: Redis is the primary bottleneck
Observation: measured latency improved by 4%
Evidence: benchmark artifact ref + test result refs
Decision: reject hypothesis
Route: FAILED (explicit research conclusion)
```

The semantic records belong to the Research layer. Core may retain opaque,
secret-free evidence references or generic Operation/Event provenance, but it
does not score hypotheses or decide that a route is false.

## 10. M4-021 Handoff Meaning

M4-021 is best understood as:

> Agent A leaves an Exploration Node with durable Core state; Agent B resumes
> from the explicit source state and continues the next node or route.

It is not a transfer of a provider session. The durable chain is:

```text
Node N -> Execution E1 -> Checkpoint C1 -> Handoff -> Resume -> Execution E2 -> Node N+1
```

The provider thread/session may end, persist separately, or be recreated.
Pong keeps these identities distinct:

```text
provider thread/session != runtime identity != Pong agent_id
Pong agent_id != transport session_id != incarnation
exploration identity != all of the above
```

Handoff does not transfer Workspace lease authority. The receiving Execution
must acquire its own valid lease and satisfy the existing ownership,
revision, generation, and recovery checks.

## 11. Evidence, Comparison, Selection, and Promotion

The Composition/Research layer may compare routes using outcome, evidence,
cost, changes, risk, performance, and confidence. A comparison should be a
reproducible view over explicit records, or a durable report containing its
input references and criteria.

Evaluation may be human, scripted, benchmark-based, or provider-assisted, but
the evaluator and evidence provenance must be explicit. Numeric scores are
optional metadata, not a Core ordering contract.

Selection records who chose a route/candidate, under which criteria, and why.
There is no global winner field and no automatic ranking. Promotion is an
explicit decision followed by a separately authorized Core operation such as
materialization or publication. A selected route can still be superseded by a
later decision without rewriting history.

## 12. Core Boundary

| Concept | Pong Core | Composition / Research | External Provider |
| --- | --- | --- | --- |
| Exploration identity | Validate referenced Task only | Own durable identity and purpose | May carry opaque context ref |
| Route identity | Validate referenced IDs | Own route/source/status | No authority |
| Node | Persist only generic refs if needed | Own node graph and semantics | Produces work output only |
| Fork relation | Execute materialization/resume primitives | Own explicit base-to-route relation | No fork authority |
| Hypothesis | No semantic interpretation | Own claim and lifecycle | May suggest a claim |
| Observation | No inference | Own observation record | Produces provider output |
| Evidence reference | Enforce secret-free/opaque boundaries | Own evidence catalog and provenance | Supplies result/artifact refs |
| Decision | Preserve generic audit provenance | Own route/hypothesis decision | No automatic decision |
| Execution | Authoritative lifecycle, ownership, outcome | Reference only | Requests work |
| Workspace | Authoritative lease, revision, materialization | Reference only | Uses assigned path/runtime |
| Checkpoint | Authoritative recovery anchor | Reference only | May trigger checkpoint request |
| Handoff | Authoritative durable handoff relation | Associate with node transition | Provider session is separate |
| Agent identity | Authoritative logical identity | Reference only | Provider metadata is separate |
| Provider session | Not owned | Store opaque reference only if needed | Own session/thread/process |
| Comparison | No automatic scoring | Own criteria, result, provenance | Optional input |
| Selection | No global winner | Own append-only decision | No authority |
| Promotion | Perform explicit authorized state operation | Own approval/intent | No implicit promotion |

## 13. Persistence Boundary

The following must be durable in the Exploration/Research layer:

- Exploration and Route identities and Task binding;
- parent/fork relationship and immutable source anchor;
- Node state, parent relation, and references to Core Versions/Checkpoints;
- explicit Execution and Handoff linkage;
- evidence references, observation provenance, and decision status;
- route failure, abandonment, blocking, completion, and promotion decisions;
- comparison criteria and the input record identities needed to reproduce it.

Pong Core already durably stores the authoritative Version, Snapshot,
Workspace, Execution, Checkpoint, Resume, Handoff, Operation, and recovery
records. A future composition store should reference those IDs instead of
duplicating their payloads.

The following should not be persisted by Pong Durable Core:

- provider raw chat transcripts unless an external provider archive explicitly
  owns them;
- LLM hidden reasoning or chain-of-thought;
- temporary prompts, transient tool output, cookies, tokens, or credentials;
- provider process internals and transport connection state;
- inferred scores or route success without an explicit evidence-backed
  decision.

## 14. Protocol v1.0 Boundary

Protocol v1.0 remains `FROZEN` and is sufficient for the current Core
execution, checkpoint, handoff, resume, publication, and inspection boundary.
This proposal does not add `exploration_id`, `route_id`, `node_id`,
`hypothesis_id`, or related commands to v1.0.

If a future external workflow cannot operate by composing existing commands,
the result should first be recorded as a **FUTURE CONTRACT GAP** with a
separately versioned proposal. It must not be filled by hidden fields or
provider-specific extensions to v1.0.

## 15. Historical Experiments and Compatibility

The repository's existing E1/E2 implementation and tests are retained as:

```text
E1/E2 = HISTORICAL / EXPERIMENTAL / RECONSIDERED
E3     = NOT STARTED / STOPPED
```

The prior `EXPLORATION_MODEL.md` and `EXPLORATION_E2A_CROSS_WORKSPACE_FORK.md`
are design and experiment materials. They inform this proposal but do not
ratify production behavior. No new E1/E2 expansion, Candidate/Evaluation/
Selection Core API, or migration is implied.

## 16. Migration and Compatibility Considerations

No migration is proposed in this document. If a future implementation is
approved, it must first define:

- whether the upper-layer store is repository-local or external;
- how it validates Core IDs and generation/environment scope;
- idempotency and crash boundaries for a fork or decision;
- how orphaned composition records are reconciled after a Core failure;
- redaction and secret scanning for evidence references;
- read-only compatibility with repositories that have no exploration data.

Historical E1/E2 tables must not silently become a ratified schema. Any reuse
requires an explicit migration and a separate acceptance decision.

## 17. Open Questions

- Should the Exploration/Research layer live beside Pong or in a separate
  orchestrator repository?
- Is a Node a state observation, a transition, or both, and what is the minimum
  immutable event set needed to reconstruct it?
- Should route status be an append-only decision projection rather than a
  mutable field?
- Which evidence formats can be safely referenced without copying sensitive
  provider output?
- What exact criteria make a Promotion eligible to update a target Workspace?
- Does an external operator need read-only Protocol queries for route context,
  or can it maintain its own projection from Core records?
- What contract gap, if any, remains after attempting a composition-only
  prototype with Protocol v1.0 unchanged?

## 18. Proposed Future Slice Boundary

The only proposed future direction is:

```text
Future Slice Candidate: Durable Exploration Model
Status: PROPOSAL ONLY / NOT RATIFIED
```

Before any implementation, a separate route decision must ratify scope,
acceptance, ownership, persistence, migration, and evidence strategy. A first
implementation should remain composition-layer or Core-internal and should
prove only identity, explicit source/fork references, route/node linkage,
failure preservation, and cold reopen. Candidate, Evaluation, Selection,
Comparison, and Promotion should remain outside the first implementation until
their contracts are independently accepted.

## 19. Current Route State

```text
M4-020: COMPLETE
M4-021: COMPLETE / PASS
Official Next Slice: NONE
Roadmap: NEEDS_RECONCILIATION
Runtime Integration: NOT CURRENT MAINLINE
Protocol v1.0: FROZEN / UNCHANGED
E1/E2: HISTORICAL / EXPERIMENTAL / RECONSIDERED
E3: STOPPED
Production implementation: STOPPED
```
