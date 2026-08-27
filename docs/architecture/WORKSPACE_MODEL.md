# Workspace Model

**Implementation status:** The logical metadata, epoch-lease, local-driver,
snapshot, and new-directory materialization subset is implemented internally.
The complete lifecycle, reconciliation, provider API, commit linkage, and M2
acceptance matrix are not complete. See
[`M2_WORKSPACE_SNAPSHOT_GATE.md`](../development/M2_WORKSPACE_SNAPSHOT_GATE.md).

## Definition

A workspace is a logical, mutable materialization of a project state. It has an id independent of its physical path or provider.

```text
Workspace = project + provider binding + current ref + environment revision + lease + reconciliation status
```

## Providers

The provider contract can target a local filesystem, container, sandbox, remote machine, or future Kubernetes pod. Provider capabilities declare whether snapshots, process isolation, network controls, and atomic replacement are available.

The current local driver records a canonical absolute locator but continues to
use `workspace_id` as identity. It rejects the repository root, `.pong`, and
paths below `.pong`; a locator containing configured secret material also fails
before metadata creation. This locator is host-specific internal metadata, not
a portable public handle. No non-local driver is implemented.

## Ownership and sharing

An agent may own many workspaces. A workspace may have multiple members for observation or collaboration, but one active write lease is the default. Shared writes require an explicit policy, operation serialization, and conflict detection. The safe recommendation is one agent per writable workspace and branches for parallel work.

## Branch binding

Branches are project-scoped refs. A workspace records the branch it currently materializes, but the branch is not owned by the workspace. Checkout changes the workspace binding after lease and cleanliness checks; it does not rewrite branch history.

## Lifecycle

`created -> preparing -> ready -> active -> paused -> reconciling -> archived`.

Crash recovery may place a workspace in `reconciling` until provider state and Pong metadata agree. Destruction requires explicit retention handling for snapshots, artifacts, and events.

The current implementation creates `created` records and permits guarded status
updates, but it does not yet implement or audit the full lifecycle transition
graph. `ready`, `active`, and `paused` are rejected unless both a materialized
head and an existing same-project environment are bound. Creation also rejects
an optional environment reference that is missing or belongs to another
project.

## Leases and revisions

Each local mutation presents a lease token containing workspace ID, agent ID,
epoch, and expiry. Acquisition conflicts while another lease is unexpired.
After release or expiry, a takeover increments the epoch; an old token cannot be
renewed or used for a workspace update. The workspace row also carries a
monotonic revision, and updates compare the caller's expected revision before
advancing state.

Lease time is currently supplied by the caller as milliseconds. A public clock
and multi-process protocol contract, clock-skew policy, and cold-restart matrix
remain M2 work.

## Local snapshots and materialization

The bounded local snapshot represents directories and regular file bytes only.
Paths are canonical UTF-8 relative paths with `/` separators and portable-name
validation; traversal, Windows-reserved names, control characters, symlinks,
reparse points, and other filesystem object kinds fail closed. Entries are
sorted bytewise before canonical encoding. File blobs and the tree manifest are
stored in separate typed CAS domains and the manifest records workspace,
project, and redaction-profile identity.

Materialization verifies the manifest and every referenced blob, writes into a
new sibling temporary directory, syncs the tree, and renames it to a destination
that must not already exist. It does not replace or merge an existing working
directory. File mode, ownership, ACL, symlink, sparse-file, and hard-link
semantics are not captured by this slice.

## Migration

Migration exports a provider-neutral manifest, refs, required objects, and environment requirements, then materializes them on a new provider. Runtime-specific process state is advisory unless the provider supports a checkpoint contract.

## Invariants

- A workspace has at most one current head and one write lease.
- Physical path is metadata, never identity.
- A workspace cannot claim `ready` while its head or environment revision is unknown.
- Uncommitted changes remain workspace state and are not silently converted into a branch commit.
- A stale/expired lease or stale workspace revision cannot advance the head.
- A failed snapshot may leave unreachable verified CAS objects, but it cannot
  advance workspace metadata.
