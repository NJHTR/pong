# Architecture Scope Reconciliation

Date: 2026-10-03

## Current state

```text
Current HEAD: d4cdbf1 feat: add exploration route fork lifecycle
Branch: dev
Remote: origin/dev at d4cdbf1
```

The current formal roadmap remains M4 provider-neutral runtime work. The
authoritative `docs/roadmap/NEXT_TASK.md` names M4-020 native regression
checkpoint finalization as the next official action and explicitly states that
Exploration, Route, Candidate, Evaluation, Search, and Selection are research
or composition-layer concepts, not Pong Core domain entities.

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
Current official next Slice: M4-020 checkpoint / roadmap follow-up
```

This document does not start M4-020, alter its checkpoint, rerun its native
workflow, or begin E3. It records the scope reconciliation only.

