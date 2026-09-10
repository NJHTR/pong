# ADR-M3-005: Cross-Workspace Base Version Reference

- **Status:** `Proposed / Internal M3`
- **Date:** 2026-09-09
- **Scope:** M3-SLICE-003C contract design only
- **Implementation:** none; no production code, DDL, migration, or runtime API

## Context

M2 Versions are immutable and Workspace-owned. M3 parallel execution requires
several isolated Workspaces to start from one stable Version without copying
that Version or weakening existing scope checks.

## Problem

The existing `Execution.base_version_id` validator requires the Version and
Execution Workspace to match. That preserves M2 lineage but cannot represent
`E1/W1`, `E2/W2`, and `E3/W3` all starting from `V100[W1]`.

## Decision

Adopt an **Execution-level immutable Base Reference** as the recommended future
model. A Base Reference points to an existing immutable Version as a read-only
starting source. It is distinct from Version ownership, Snapshot content,
Execution Workspace, Workspace Head, Version Head, and Version parentage.

The existing M2 `Version.workspace_id`, `parent_version_id`, and same-Workspace
`Execution.base_version_id` meanings remain unchanged. Cross-Workspace use
must be represented by a separately named additive field/relation or equivalent
explicit attachment; it must not be implemented by weakening current
validation.

## Goals

- Share one immutable stable base across isolated Workspaces.
- Preserve Workspace-local Version ownership and parent lineage.
- Keep Snapshot/CAS reuse read-only and deterministic.
- Preserve lease, revision, recovery, security, and legacy boundaries.

## Non-Goals

No production implementation, schema/DDL, migration, scheduler, orchestration,
provider, Branch, Merge, Rebase, Candidate, Approval, Agent State, Memory,
Skill, Automation, CLI, SDK, UI, or automatic conflict resolution. Merge
remains **FUTURE**.

## Current Conflict and Ownership

Version ownership stays `Version.workspace_id`. A Base Reference does not
transfer ownership or authority. Version content remains the immutable Snapshot
identified by `Version.snapshot_id`. An Execution writes only to its explicit
Workspace and requires that Workspace's lease and revision guard.

## Base Reference Validation

The target Version must be durable, immutable, healthy, and compatible by
project, environment, generation, and migration. Missing, corrupt, or
cross-scope references fail closed. The reference carries no secrets or raw
provider context and cannot mutate the source Version or Workspace.

## Parallel Workspace Model

The recommended shape is one writable Execution per Workspace. E1/W1, E2/W2,
and E3/W3 may share Task and Base Reference V100 while retaining independent
leases, revisions, Snapshot Heads, Version Heads, Operations, Checkpoints,
Rollbacks, and physical trees.

## Workspace Head and Version Head

`Workspace.head` remains a per-Workspace Snapshot digest. Version Head remains
an explicit per-Workspace logical selection. E2's Base Reference to V100 cannot
change W1.head or W1 Version Head; W2 selection remains W2-local and guarded.

## Snapshot, Restore, and Rollback

S100/CAS content may be materialized or diffed concurrently into W1/W2/W3.
Restore and rollback operate on the target Workspace only. Rolling W2 back to
V100 does not modify W1, V100 ownership, W1 heads, or sibling Executions.

## Version Graph and Parent

`parent_version_id` remains same-Workspace immutable lineage. A Base Reference
is not a graph edge. Therefore `V201[W2]` must not infer
`parent_version_id = V100[W1]`; it uses null or a valid W2 parent under M2.
Cross-Workspace lineage and Merge are future contracts.

## Checkpoint, Resume, and Handoff

Checkpoint reuse across Workspaces remains open and does not gain authority
implicitly. Resume creates a new Execution carrying an explicit Base Reference.
Handoff may carry that reference but never transfers Version ownership or a
Workspace lease; the target revalidates all scope and lease conditions.

## Lease and Revision

Leases and revisions remain Workspace-local. Independent Workspaces can mutate
concurrently. A Base Reference grants no write authority over its source.

## Failure Boundary and Recovery

Reference, materialization, provider, lease, revision, and persistence failures
produce complete old state, complete new state, or deterministic conflict/
unknown. No phantom base success is allowed. Cold reopen preserves each
Execution and reference independently; exact retry is deterministic.

## Security

Only opaque Version IDs and redacted integrity metadata are permitted. Secrets,
tokens, credentials, cookies, private keys, raw prompts, and transcripts are
excluded.

## Legacy

M1 v0.1.0 and all M2 semantics remain unchanged. Legacy repositories receive no
synthetic Base References and no rewritten Version rows or parent edges.

## Migration

Future implementation may require one minimal additive Execution-level field or
relation. Existing rows remain untouched; backfill is not inferred from
Workspace Head, Version Head, timestamps, or latest rows. If one relation is
insufficient for the accepted retry/recovery contract, implementation must stop
with `SCHEMA_GAP` rather than add multiple entities opportunistically.

## Alternatives

- Strict Workspace-local duplication: safe but duplicates stable baselines.
- Independent BaseReference entity: deferred unless Execution-level storage is
  insufficient; otherwise it introduces unnecessary core schema.
- Project-scoped Versions: rejected because it changes M2 ownership, graph,
  Head, migration, and legacy semantics.

## Recommended Model

Model B is recommended:

```text
V100[W1] -- BaseRef(E1/W1) --> V101[W1]
       \-- BaseRef(E2/W2) --> V201[W2]
        `- BaseRef(E3/W3) --> V301[W3]
```

The Base Reference arrows are read-only execution anchors, not Version parent
edges.

## Open Decisions

Exact additive storage; reference retention/deletion; Checkpoint reuse;
cross-Workspace lineage; Version Head selection; cross-project policy;
reconciliation persistence; and future Merge/Branch/Rebase.

## Risks

Confusing Base Reference with parentage could corrupt graph semantics. Weak
scope validation could expose cross-project content. A future durable relation
could accidentally become a second Version or event system.

## Status

`M3-SLICE-003C = CONTRACT_READY` after the companion proposal documents,
ignored contract tests, and limited validation pass.
