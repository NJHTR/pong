# Durable Agent Work Graph Proposal

**Date:** 2026-10-05
**Baseline:** `4ae4a7a` (`dev`)
**Status:** `PROPOSED ARCHITECTURE / NOT RATIFIED / NOT IMPLEMENTATION`

This proposal evaluates whether Pong should grow from durable Agent execution
infrastructure into an evidence-backed Agent Work Graph. It is a product and
architecture boundary study. It does not create M4-022, modify Protocol v1.0,
add a schema, or authorize production implementation.

## 1. Executive Summary

The market already provides memory, handoff, checkpoints, worktrees,
background agents, parallel agents, and session continuity. Pong should not
compete by becoming another memory database, provider manager, or checkpoint
utility.

The unmet problem is provenance across agent work:

```text
Why did we get here?
What did we try?
Which paths failed and why?
What evidence supports the current state?
Which Agent produced it?
Why was one route promoted over another?
Where can another Agent safely resume?
```

The recommended product direction is:

> **Agent Work Provenance Infrastructure, expressed as an evidence-backed
> Work Graph.**

`Work Graph` is the umbrella concept. `Exploration` and `Route` are research
subgraphs within it. Pong remains the authority for durable execution and
state; the Research/Composition layer owns hypotheses, observations,
comparisons, decisions, and promotion intent.

This direction is differentiated only if it preserves factual provenance and
reproducible links to Pong's Version, Execution, Checkpoint, Workspace, and
Artifact records. A graph without evidence or durable Core linkage would be
another planning or memory product and would not justify expansion.

## 2. Market Findings

The following market context is the input to this study, not a claim that Pong
must reproduce these products:

| Product/category | Existing capability | Boundary implication for Pong |
| --- | --- | --- |
| Cursor | Background agents, parallel agents, worktrees, checkpoints, multi-root workspaces, multitask | Do not rebuild agent scheduling, worktrees, or checkpoint UX |
| Codex | Long-running tasks, multiple agents, parallel work, command-center workflows | Do not become another provider command center |
| GitHub Copilot | Repository memory, user memory, session persistence, continuation | Do not become a preference or prompt memory store |
| Claude Code | Project memory, user memory, `CLAUDE.md`, cross-session context | Do not compete with provider-specific memory |
| AICTX / Drift class | Repo-local continuity, handoff, decisions, failures, next actions | Continuity is real demand, but is not sufficient differentiation |
| Git | Content and change history | Add causal work provenance, not a replacement VCS |

The market validates the problem of continuity and parallel work. It does not
validate a new generic memory product. The defensible gap is the connection
between factual work state, failed alternatives, evidence, agent execution,
and promotion decisions across providers.

## 3. Problem Statement

Pong's current infrastructure records reliable work mechanics:

- Repository and environment identity;
- Workspace materialization, leases, revision/CAS, and status;
- immutable Snapshot and Version lineage;
- Execution and Operation ownership, outcomes, and recovery;
- Checkpoint, Resume, Handoff, Rollback, and cold reopen;
- provider-neutral JSON Lines control and durable inspection.

M4-020 established native runtime reproducibility. M4-021 established a real,
user-controlled Codex-to-Claude continuation with durable handoff, fresh
process inspection, and cold reopen.

That is enough to answer **how work was executed**. It does not yet express:

- the intent and hypothesis that motivated a route;
- why a route was forked;
- which failed path should not be repeated;
- which observations support a decision;
- why a current Version was promoted;
- how to compare alternatives without rewriting their history.

## 4. Product Positioning

| Position | Market competition | Technical asset | Git difference | Future value | Decision |
| --- | --- | --- | --- | --- | --- |
| A. Durable Execution Infrastructure | Low differentiation; many agent runtimes provide execution | Strong today: Core lifecycle, recovery, leases, versions | More than Git process history, but mostly operational | Reliable substrate, not a complete product thesis | Retain as foundation, insufficient as final positioning |
| B. Durable Agent Exploration Infrastructure | Some differentiation, but overlaps research planners and memory tools | Adds route/fork/evidence composition over existing Core | Preserves alternatives and failed research paths | Useful for long-running investigation | Valid bounded domain, not the complete framing |
| C. Agent Work Provenance / Exploration Graph | More distinct if evidence-linked and provider-neutral | Combines Core facts with explicit causal research relations | Explains why a state exists, not only what changed | Supports comparison, handoff, recovery, and promotion | **Recommended** |

The recommendation is C, with B as a named capability inside the Work Graph.
The recommendation is conditional: it is not a license to add graph entities
to Pong Core before contracts, evidence, and ownership are accepted.

## 5. Design Goals

- Answer `why did we get here?` using durable, inspectable references.
- Preserve successful, failed, blocked, and abandoned routes.
- Link research claims to actual Execution, Version, Snapshot, Checkpoint,
  Artifact, and Evidence records.
- Make forks explicit and cheap without copying a Repository.
- Allow Codex, Claude, Copilot, Cursor, or another runtime to act as an
  external executor without provider-specific Core logic.
- Keep comparisons and decisions auditable without automatic ranking.
- Let an external Composition/Research layer evolve without destabilizing Core.

## 6. Non-goals

Pong must not become:

- a Vector DB, RAG memory, prompt archive, or chat-history warehouse;
- a store for hidden model reasoning or private chain-of-thought;
- a Codex, Claude, Cursor, or Copilot manager;
- a scheduler, LLM judge, automatic ranking engine, or global winner service;
- a Git replacement or branch/merge implementation;
- a provider adapter framework;
- a new Protocol v1.0 surface;
- an automatic promotion or deletion mechanism.

## 7. Proposed Work Graph

The Work Graph is an upper-layer provenance graph over existing Pong Core
graphs:

```text
Task
  |
  +-- Work Graph
        |
        +-- Exploration / investigation scope
        |     |
        |     +-- Route A
        |     |     +-- Work Node A1
        |     |     +-- Work Node A2
        |     |     +-- failed Execution / Checkpoint
        |     |
        |     +-- Route B (fork from A1/A2 source)
        |           +-- Work Node B1
        |           +-- Work Node B2
        |
        +-- Core references:
              Execution -> Workspace -> Version -> Snapshot
              Checkpoint -> Resume -> Execution
              Handoff -> next Execution
              Artifact / evidence references
```

The Work Graph answers causal and research questions. Pong Core remains the
authority for the referenced records and their integrity.

## 8. Work Graph Concepts

| Concept | Purpose | Identity/lifecycle | Owner | Persistence |
| --- | --- | --- | --- | --- |
| Task | Long-lived unit of work | Existing Core identity/lifecycle | Pong Core | Durable |
| Work Graph | Provenance scope over one Task | Proposed upper-layer aggregate | Research/Composition | Durable |
| Exploration | Investigation scope or question | Proposed relation inside Work Graph | Research/Composition | Durable |
| Route | One alternative path | Explicit source, status, terminal refs | Research/Composition | Durable |
| Work Node | Factual research state and decision point | Parent/route relation plus Core refs | Research/Composition | Durable |
| Fork | Base-node to new-route relation | Immutable source anchor | Research/Composition | Durable |
| Hypothesis | Claim under investigation | Explicit claim and status | Research/Composition | Durable when decision-relevant |
| Action | Intended experiment or work step | Linked to one or more Executions | Research/Composition | Intent durable; process is Core |
| Observation | Factual result | Provenance and evidence refs | Research/Composition | Durable when retained |
| Evidence | Artifact/test/observation support | Opaque, secret-free reference | Research/Composition | Durable reference |
| Decision | Explicit interpretation or choice | Actor, reason, criteria, refs | Research/Composition | Durable |
| Execution | Concrete agent run | Existing lifecycle and ownership | Pong Core | Durable |
| Workspace | Writable materialization | Existing lease/revision lifecycle | Pong Core | Durable |
| Version | Immutable logical state | Existing parent/version lifecycle | Pong Core | Durable |
| Snapshot | Content-addressed state | Existing immutable identity | Pong Core | Durable |
| Checkpoint | Recovery anchor | Existing immutable relation | Pong Core | Durable |
| Handoff | Execution-to-execution context relation | Existing durable lifecycle | Pong Core | Durable |
| Comparison | Evidence-backed view of alternatives | Criteria and inputs explicit | Research/Composition | Durable report or reproducible view |
| Selection | Scoped choice | Append-only decision record | Research/Composition | Durable |
| Promotion | Adoption intent plus explicit Core action | Never deletes alternatives | Research/Composition + Core action | Durable |

## 9. Work Node Model

A Work Node is a factual work-stage record, not a process and not a file tree.
It may contain or reference:

```text
Intent / research question
Hypothesis reference
Action reference
Observation reference
Evidence references
Decision reference
Base Version or Checkpoint reference
Execution references
Artifact references
Current route status
```

These are conceptual relations, not an approved schema. The Node must not be
equated with:

- `Execution`: one node can have multiple attempts or a resumed execution;
- Git commit: a Version may be published without being a research decision;
- Snapshot: a Snapshot is content state, not causal meaning;
- Workspace: a node can be materialized in different Workspaces over time.

The minimum useful invariant is that a Node can answer which factual Core
records support it and which explicit decision moved work onward.

## 10. Route and Fork Semantics

`fork(base_node)` means:

```text
Base Work Node
  -> immutable Version or Checkpoint source
  -> new Route identity
  -> new Work Node
  -> separate writable Workspace
  -> new Execution
```

Fork rules:

- The old Route and Node history remain immutable and may continue running.
- Immutable Version, Snapshot, Checkpoint, and evidence references may be
  shared read-only.
- A new route receives a new identity and explicit source relation.
- A new writable Workspace and Execution are required for independent work.
- Version publication and parentage use existing Core semantics; route
  membership is not inferred from Version ancestry.
- Evidence and decisions may be inherited by reference, but inherited facts
  must be marked as inherited rather than copied as new observations.
- Forking never clones the Repository, database, credentials, or provider
  session.

## 11. Failure Semantics

Failure is valuable provenance, not garbage collection. Recommended route
statuses are:

```text
ACTIVE -> BLOCKED
ACTIVE -> FAILED
ACTIVE -> ABANDONED
ACTIVE -> COMPLETED
COMPLETED -> PROMOTED
```

`FAILED` means an explicit research conclusion or exhausted route, not merely
one provider error. `BLOCKED` means work cannot proceed for an external or
policy reason. `ABANDONED` is an explicit human/orchestrator action.
`COMPLETED` means the route met its stated stopping condition. `PROMOTED`
records adoption and never deletes alternative routes.

Core Execution/Operation failures remain authoritative at the Core layer. A
Research layer may later decide that a hypothesis failed, supported, or was
inconclusive. It must not infer that meaning from a callback or timestamp.

## 12. Evidence Model

Evidence should be factual and referenceable:

- test result or benchmark record;
- artifact digest or report identifier;
- Observation with actor, time, and input reference;
- Version/Snapshot/Execution/Checkpoint identity;
- external review or approval reference.

Evidence records should include provenance, scope, and integrity references.
Pong Core may enforce secret-free opaque references and preserve generic audit
events. The Research layer owns evidence interpretation. Hidden model
reasoning, credentials, and raw provider conversations are outside the Core
evidence model.

## 13. Handoff Model

M4-021 demonstrates a Work Graph transition, not session migration:

```text
Work Node B3
  -> Codex Execution E1
  -> Checkpoint C1
  -> Handoff
  -> Claude Execution E2
  -> Work Node B4
```

The provider may change while the Work Node and factual Core references remain
stable. These identities stay distinct:

```text
provider session/thread
runtime identity
Pong agent_id
transport session_id
incarnation
Work Graph / Node identity
```

Handoff does not transfer lease authority. The new Execution obtains its own
lease and follows existing ownership, revision, generation, and recovery
rules.

## 14. Comparison, Selection, and Promotion

`Compare(Route A, Route B)` should compare explicit facts such as:

- outcome and validation strength;
- evidence coverage and confidence;
- cost and elapsed work;
- risk and scope of changes;
- performance and operational impact;
- remaining unknowns.

The comparison is a Research/Composition report or reproducible view. It is
not an automatic score in Pong Core.

Selection records the actor, criteria, evidence, and reason for choosing a
route or candidate. Promotion is a separate action:

```text
Route A / Route B / Route C
          -> comparison
          -> explicit selection
          -> promotion decision
          -> authorized Core materialization/publication
```

Promotion does not delete, merge, or rewrite unselected routes. Pong Core may
perform the explicit resulting state operation, but does not decide which
route is best.

## 15. Git Relationship

Git primarily answers:

```text
What changed?
Which content objects and commits exist?
```

The proposed Work Graph answers:

```text
Why did we get here?
What alternatives were tried?
What failed and why?
Which Agent and Execution produced this state?
What evidence supports the decision?
What can be resumed safely?
Why was this route promoted?
```

Pong must not replace Git. A Version may correspond to a published filesystem
state with no Git commit, and a Git commit does not by itself explain the
research decision or failed alternatives that led to it.

## 16. Core / Research / Provider Boundary

| Concept | Pong Core | Research / Composition | Provider |
| --- | --- | --- | --- |
| Work Graph identity | Validate Task and scope refs | Own identity and purpose | Opaque context only |
| Work Node | Validate referenced Core IDs | Own node graph and semantics | Produces work result |
| Route / Fork | Run materialize/resume primitives | Own route identity and fork reason | No authority |
| Execution | Own lifecycle, ownership, outcome | Reference | Requests work |
| Workspace / Version / Snapshot | Own state, leases, lineage | Reference | Uses assigned state |
| Checkpoint / Handoff | Own recovery and transfer records | Associate with node transition | Provider session is separate |
| Hypothesis / Observation | No semantic inference | Own claims and factual reports | Optional input |
| Evidence reference | Enforce redaction/secret boundary | Own catalog and provenance | Supplies artifacts/results |
| Decision | Preserve generic audit provenance | Own interpretation and decision | No automatic decision |
| Comparison / Selection | No ranking or global winner | Own criteria and choice | Optional evaluator |
| Promotion | Execute explicit authorized Core action | Own promotion intent | No implicit promotion |
| Provider session | Not owned | Opaque reference only | Own process/thread/session |

## 17. Persistence Boundary

Durable Work Graph facts should include:

- Work Graph, Exploration, Route, Node, and Fork identities;
- Task binding and parent/source relations;
- route status and explicit failure/abandonment/blocking decisions;
- Version, Snapshot, Checkpoint, Execution, Workspace, and Handoff references;
- action, observation, evidence, comparison, selection, and promotion records;
- the provenance needed to reproduce a comparison.

Pong Durable Core should not persist:

- raw provider transcripts forever;
- hidden model reasoning or private chain-of-thought;
- temporary prompts and ephemeral tool state;
- provider credentials, cookies, or tokens;
- provider session internals as authoritative Work Graph identity;
- automatic scores without explicit criteria and evidence.

## 18. Protocol v1.0 Boundary

Protocol v1.0 remains `FROZEN / UNCHANGED`. Existing commands can compose the
initial workflow:

```text
Execution + Workspace + Version + Checkpoint + Handoff + Resume
```

No `work_graph_id`, `node_id`, `route_id`, `hypothesis_id`, or new Work Graph
command is added to v1.0. If a composition-only prototype proves that a
required workflow cannot be expressed through current queries and commands,
record a `FUTURE CONTRACT GAP` and propose a separately versioned contract.

## 19. Real-world Scenarios

### Scenario A: Performance optimization

```text
Task: optimize API P95 latency
Route A: DB indexing -> failed
Route B: Redis caching -> partial improvement
Route C: connection pooling -> successful
Selection: C, supported by benchmark and regression evidence
```

Pong should answer what was tried, why A was rejected, why B was not promoted,
and which evidence supports C. It should not rank A/B/C automatically.

### Scenario B: Bug investigation

Three routes investigate race condition, cache invalidation, and
serialization. Each retains its Work Nodes, Executions, evidence, failures,
and current conclusion. Parallel Workspaces and Core leases remain independent.

### Scenario C: Architecture decision

Kafka, RabbitMQ, and Redis Streams are separate routes. Pong retains evidence,
comparison facts, decision criteria, and promotion history. The final choice
is an explicit external decision, not a Core-selected winner.

### Scenario D: Agent handoff

Codex leaves Route B3 at Checkpoint C1. Claude resumes from C1 in a new
Execution and continues Route B4. Claude does not need the old provider
session; it needs durable Work Node, Core references, and evidence context.

### Scenario E: Context failure

After a provider crash, a new Agent reads the Work Node, Checkpoint, evidence,
decision status, and source Version. It resumes through existing Core Resume
semantics, while the failed Execution remains preserved history.

## 20. Market Differentiation

Pong should not compete with Cursor's worktree/parallel-agent UX, Codex's
command center, Copilot's repository/user memory, Claude Code's provider
memory, or AICTX/Drift-style continuity alone. Those are adjacent surfaces.

The proposed differentiation is the factual join across:

```text
Evidence-backed Work Graph
+ branchable exploration routes
+ immutable Version/Snapshot lineage
+ Execution and Agent linkage
+ durable failure history
+ provider-neutral handoff/recovery
+ explicit comparison and promotion provenance
```

This is a potential technical and product boundary, not yet a validated
market moat. A future prototype must test whether developers use provenance to
avoid repeated work and justify decisions, rather than merely requesting
another memory view.

## 21. Open Questions

- Should the Work Graph store live beside Pong or in an external orchestrator?
- Is a Work Node a state, a transition, or a pair of immutable facts?
- Which evidence formats are safe to reference without copying sensitive
  provider output?
- How are composition records reconciled after a Core operation is unknown?
- Should route status be append-only decisions projected into a current view?
- What exact promotion operation is safe for a selected route?
- Can the first prototype remain entirely outside Protocol v1.0?
- What user research demonstrates value beyond existing memory and handoff
  products?

## 22. Future Slice Proposal

```text
Future Candidate: Durable Agent Work Graph
Status: PROPOSAL ONLY / NOT RATIFIED
```

If later approved, the first bounded slice should prove only:

1. Work Graph/Exploration/Route/Node identity and Task scope;
2. explicit fork source references;
3. linkage to existing Execution, Version, Checkpoint, and Handoff records;
4. preservation of failed and abandoned paths;
5. cold-reopen and reconciliation behavior;
6. evidence references without secret or hidden-reasoning persistence.

Comparison, Evaluation, Selection, Promotion, provider adapters, and new
Protocol commands should remain outside that first implementation until their
contracts are independently accepted.

## 23. Current Route State

```text
M4-020: COMPLETE
M4-021: COMPLETE / PASS
Official Next Slice: NONE
Roadmap: NEEDS_RECONCILIATION
Runtime Integration: NOT CURRENT MAINLINE
E1/E2: HISTORICAL / EXPERIMENTAL / RECONSIDERED
E3: STOPPED
Protocol v1.0: FROZEN / UNCHANGED
Production implementation: STOPPED
```

This proposal is an architecture/product study only. It is not M4-022
implementation and does not ratify a future Slice.
