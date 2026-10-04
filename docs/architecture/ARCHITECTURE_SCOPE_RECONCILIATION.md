# Architecture Scope Reconciliation

Date: 2026-10-04

## Current state

```text
Current HEAD: 1eaa15e docs: reconcile roadmap after m4-020
Branch: dev
Remote: origin/dev at 2880d33
```

M4-020 is now `PASS / COMPLETE`, and its formal checkpoint `d6e9dae` is an
ancestor of the current HEAD. M4-021 is now the single ratified post-M4-020
Slice, with execution blocked by provider service availability. Its scope is
limited to provider-neutral evidence over the existing transport and frozen
Protocol v1.0; it does not authorize provider adapters, Runtime Integration,
or Core schema changes.
Exploration, Route, Candidate, Evaluation, Search, and Selection remain
research or composition-layer concepts, not Pong Core domain entities.

## Commit timeline

The relevant history is:

```text
66fda97 2026-09-26  checkpoint: exploration candidate route semantics
f0b7c01 2026-09-26  Revert "checkpoint: exploration candidate route semantics"
ac1696b 2026-09-27  docs: restore Pong core architecture boundary
f17e1e4 2026-10-03  docs: define exploration route model
cd0f1c4 2026-10-03  feat: add exploration and route relations
d4cdbf1 2026-10-03  feat: add exploration route fork lifecycle
```

`ac1696b` is earlier than `f17e1e4`, `cd0f1c4`, and `d4cdbf1`. It does not
rewrite or revert those later commits, but it is the earlier explicit scope
correction that governs their interpretation. `f0b7c01` is the only standard
revert of the earlier Candidate/Route checkpoint; no revert of E1 or E2 has
been performed.

## Scope decisions

### Exploration model

`f17e1e4` remains a design document describing a possible composition model.
Its own status says that no production code, schema, or Protocol v1.0 change is
authorized by the design slice. It is not an authorization to expand Pong
Core.

### E1

`cd0f1c4` added `explorations`, `routes`, and `route_executions` to the current
working implementation and tests. The implementation is durable and currently
passes its focused and full regression coverage, but it was added after the
`ac1696b` boundary correction. It must therefore be treated as an existing,
unfrozen experimental compatibility surface, not as a newly ratified Core
contract.

### E2

`d4cdbf1` composes E1 with existing Version, Checkpoint, Workspace,
Materialization, Execution, Handoff, Resume, Rollback, and Operation APIs. It
preserves Version parent scope and Protocol v1.0, but it extends the same
unfrozen Exploration/Route surface. E2 is retained as historical implementation
and evidence; no further Core expansion is authorized by this reconciliation.

### E3

```text
E3 status: NOT STARTED / STOPPED
```

Candidate, Evaluation, and Selection schemas or Core APIs must not be added.
They remain external Research/Composition Layer responsibilities until a
separate scope decision explicitly supersedes the current roadmap boundary.

## Current classification

```text
Exploration in Pong Core: EXPERIMENTAL / RECONSIDERED
Route in Pong Core: EXPERIMENTAL / RECONSIDERED
Candidate: NOT IMPLEMENTED
Evaluation: NOT IMPLEMENTED
Selection: NOT IMPLEMENTED
```

The evidence supports Option B from the reconciliation request: retain the
already-pushed E1/E2 commits and their compatibility tests, stop extending
them, and require a dedicated migration or removal decision before treating
them as formal Core capability. No public-history rollback is appropriate in
this slice. A future external orchestrator may use Pong's existing durable
substrate, but that does not authorize new Exploration-domain entities in Core.

## Protocol and repository boundaries

Protocol v1.0 contains no Exploration, Route, Candidate, Evaluation, or
Selection commands or resources and remains unchanged. Existing provider and
Windows untracked evidence is unrelated and remains untouched. `easyCode` and
`D:\\bs\\seekwd` are not touched.

```text
Production code changed in this slice: NO
Schema/migration changed in this slice: NO
Protocol v1.0: UNCHANGED
```

## Official next slice

```text
Current official next Slice: M4-021
M4-021: RATIFIED / BLOCKED ON PROVIDER ENVIRONMENT
```

Runtime Integration is not a current implementation Slice. The existing local
Runtime Identity Adapter proves stable logical `agent_id` reuse and ephemeral
adapter sessions while keeping provider `thread_id`, transport sessions, and
incarnation separate. Protocol v1.0 is sufficient for the current
`register_agent` boundary; no Protocol contract gap is established.

This document ratifies the route only; it does not start M4-021, alter its
provider gate, rerun M4-020, or begin E3. Provider execution remains blocked
until the declared Claude service is available.
