# E1 Exploration / Route Relations Result

Date: 2026-10-03

## Scope

E1 adds only the durable relations needed to name an Exploration, create
Routes from an explicit Version or Checkpoint source, and attach existing
Executions to a Route. Version, Snapshot, Execution, Checkpoint, Workspace,
Handoff, Resume, Rollback, and Protocol v1.0 semantics are unchanged.

## Durable model

- `explorations` stores `exploration_id`, `task_id`, `created_by`, optional
  `purpose_ref`, and `created_at`.
- `routes` stores `route_id`, `exploration_id`, the unified `source_kind` /
  `source_id` anchor, explicit route `status`, optional terminal Version,
  creator metadata, and creation metadata.
- `route_executions` stores explicit Route-to-Execution membership. An
  Execution can be unassigned or attached to one Route; membership is never
  inferred from Version ancestry, Workspace Head, Agent, or timestamps.
- `source_kind=version` resolves to an existing Version in the Exploration
  Task project. `source_kind=checkpoint` resolves to an existing Checkpoint
  in the Exploration Task and validates its Version and Workspace scope.

The tables use the existing additive schema initialization and validation
mechanism. No second migration or bootstrap format was introduced.

## Verification

Command:

```text
cargo test --test exploration_route --locked
```

Result: PASS, 2 tests.

The focused suite proves:

- fresh Task, Exploration, Version, Checkpoint, and two Route records;
- two Routes can share the same explicit Version source;
- a Checkpoint source remains an explicit source anchor;
- Execution membership is explicit and idempotent;
- Route status remains route intent and is not changed by Execution state;
- changed Route payload under the same ID is rejected;
- missing or cross-scope source references are rejected;
- all relations survive Repository close and cold reopen.

Quality gates completed for this slice:

```text
cargo fmt --all -- --check       PASS
cargo check --all-targets --locked PASS
cargo clippy --all-targets --all-features --locked -- -D warnings PASS
cargo test --all --locked        PASS
git diff --check                 PASS
```

## Boundary confirmations

- Route fork composition: NOT IMPLEMENTED in E1; reserved for E2.
- Candidate / Evaluation / Selection: NOT IMPLEMENTED.
- Protocol v1.0: UNCHANGED.
- Provider runtime and provider adapters: NOT TOUCHED.
- `C:\Users\NJHTR\IdeaProjects\easyCode`: NOT TOUCHED.
- `D:\bs\seekwd`: NOT TOUCHED.
- Existing provider/Windows untracked files: NOT TOUCHED.
