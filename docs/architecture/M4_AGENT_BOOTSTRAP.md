# M4 Agent Bootstrap And Reconnect

**Status:** `LOCAL BOOTSTRAP IMPLEMENTED / REMOTE DISCOVERY DEFERRED / PROTOCOL-FROZEN`

This slice answers how a new Agent Runtime finds a local Pong Core and
reconnects to durable state. It does not add an Agent platform, scheduler,
provider adapter, or new Core entity.

## Current Reality

The implemented local Runtime paths are explicit or project-discovered:

```text
Runtime host
  -> pong-agent-protocol [--project-root <project>]
  -> (or no arguments from a project cwd)
  -> local JSON Lines transport
  -> Pong Core owner
  -> Repository (.pong) and Workspace bindings
```

The launch layer now resolves a nearest project `.pong/bootstrap.json`; explicit
`--repository` plus `--workspace-root` remains compatible. The focused
`pong-bootstrap initialize <repository-root>` entry point now performs the
formal local Repository/bootstrap initialization. There is still no general
`pong attach` or `pong connect` CLI, and the stdio adapter does not select a
localhost network fallback. Endpoint hints are resolved by the helper with
explicit argument, `PONG_ENDPOINT`, then metadata precedence; HTTP
authentication remains outside this file.

`.pong/bootstrap.json` is a separate, non-secret descriptor. Its v1 shape is:

```json
{
  "schema_version": 1,
  "protocol_version": "1.0",
  "repository_root": ".",
  "workspace_root": ".pong/workspaces",
  "workspace_id": null,
  "core_endpoint": null
}
```

Paths may be relative to the project root or absolute. `core_endpoint` is an
optional transport hint and is unset for stdio. The descriptor contains no
bearer token, secret, SQLite/CAS state, Agent identity, Execution state, or
history. `.pong/repository.json` remains an internal compatibility/selector
marker; durable SQLite/CAS state remains inside the Core boundary.

## Formal Initialization

Run:

```text
pong-bootstrap initialize <repository-root>
```

The entry point calls `Repository::init`, creates the Pong-owned
`.pong/workspaces` binding root, and calls `BootstrapMetadata::write` only when
`.pong/bootstrap.json` is absent. A compatible existing bootstrap is parsed
and preserved without rewriting it; an incompatible repository or malformed
bootstrap fails closed. Initialization is valid for non-Git directories and
does not overwrite user files. The stdio runtime has no listener endpoint, so
the generated descriptor keeps `core_endpoint` as `null`.

Repository and workspace durable identities are not invented by this command:
the repository marker retains its existing format/schema/storage identity, and
the optional bootstrap `workspace_id` remains unset until a Protocol workspace
is explicitly created.

## Existing Attach And Reconnect Semantics

After bootstrap resolution supplies the repository and binding-root paths, the
frozen Protocol v1.0 surface is sufficient for a minimal reconnect flow:

1. `hello` discovers the supported protocol commands and queries.
2. `register_agent` replays or creates the durable Agent identity.
3. The Runtime uses known opaque IDs with `get_workspace` and
   `inspect_execution` to inspect the Workspace and existing Execution.
4. `get_checkpoint`, `list_checkpoints`, and `resume_from_checkpoint` provide
   continuation without requiring the previous transport connection.
5. A replacement Core process reopens the same Repository; no duplicate
   Repository or Workspace is created.

The transport session is not the Agent identity. `caller_agent_id` is the
durable logical Agent identifier, while HTTP authentication sessions and the
JSONL process are ephemeral transport/runtime instances. Protocol v1.0 does
not persist a separate process-incarnation record. A Runtime that needs to
attribute a new process instance must keep that correlation outside the Core
until a future lifecycle slice defines it.

The focused regressions `zero_explicit_path_bootstrap_reconnects_after_core_restart`
and `explicit_local_bootstrap_reconnects_same_repository_workspace_and_execution`
prove both project-discovered and explicit-path flows, including marker
boundaries, repeat Agent registration, Workspace lookup, Execution inspection,
cold reopen, and the absence of duplicate Workspace/Execution rows.

## Identity Boundaries

| Concern | Current authority | Bootstrap/authentication boundary |
| --- | --- | --- |
| Pong location | Explicit launch path or `.pong/bootstrap.json` resolution | Discovery, not authentication |
| Repository | Local filesystem path plus `.pong` compatibility marker | Core storage identity |
| Project | Durable Task/Workspace project binding | Attach scope |
| Workspace | Opaque `workspace_id` and host binding reference | Attach target |
| Agent | Durable `agent_id` and registered metadata | Logical long-lived identity |
| Execution | Durable `execution_id` owned by an Agent | Existing run to inspect/resume |
| Process incarnation | Not persisted in Protocol v1.0 | Future lifecycle concern |
| Credential | HTTP transport verifier; none in Core marker or JSONL | Authentication, never discovery metadata |

Discovery answers “where is Pong?”. Authentication answers “who is calling?”.
Authorization is evaluated per Protocol request. Repository/Workspace attach
answers “which durable state is this Runtime operating on?”. These concerns
must not be collapsed into `.pong` or into a bearer token stored beside the
Workspace.

## Bootstrap Failure Semantics

The helper reports stable categories instead of collapsing all failures into a
connection error: `BOOTSTRAP_PONG_DIRECTORY_MISSING`,
`BOOTSTRAP_METADATA_MISSING`, `BOOTSTRAP_METADATA_INVALID`,
`BOOTSTRAP_REPOSITORY_MISSING`, `BOOTSTRAP_WORKSPACE_ROOT_MISSING`,
`BOOTSTRAP_PROTOCOL_INCOMPATIBLE`, and `BOOTSTRAP_ENDPOINT_INVALID`. An
endpoint that exists but cannot be reached remains a transport/client failure.

## Remaining Bootstrap Scope

The remaining gap is a richer CLI suite and remote endpoint lifecycle, not the
Core data model. The implemented offline-first descriptor contains only:

- a stable repository reference or project root;
- the default local transport/endpoint or launch command;
- an optional Workspace binding/identity hint;
- the supported Protocol version.

It must not contain bearer tokens, credentials, SQLite/CAS state, Execution
history, Snapshot data, prompts, or provider sessions. Remote endpoints and
authentication belong to the transport/security layer. The descriptor should
be separate from the compatibility selector so repository migration metadata
does not become a client credential/configuration store.

No new `Change`, `Branch`, `Fork`, Agent lifecycle, or Protocol v1.0 entity is
required to express attach/reconnect. The existing Agent, Workspace,
Execution, Checkpoint, Resume, and inspection queries are the correct Core
primitives once a host supplies the bootstrap paths and opaque IDs.

## Boundary Decision

Pong now supports **zero-explicit-path local discovery plus durable reconnect**
when `.pong/bootstrap.json` is present. It does not attempt remote service
discovery or implicit localhost selection. Those belong to a future transport
adapter slice, not to Protocol v1.0 or Workspace/Core.
