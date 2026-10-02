# M4 Agent Bootstrap And Reconnect

**Status:** `PARTIAL / BOOTSTRAP GAP / PROTOCOL-FROZEN`

This slice answers how a new Agent Runtime finds a local Pong Core and
reconnects to durable state. It does not add an Agent platform, scheduler,
provider adapter, or new Core entity.

## Current Reality

The implemented local Runtime path is explicit:

```text
Runtime host
  -> pong-agent-protocol --repository <project> --workspace-root <bindings>
  -> local JSON Lines transport
  -> Pong Core owner
  -> Repository (.pong) and Workspace bindings
```

The transport does not discover its repository or endpoint. A host must supply
both paths when starting `pong-agent-protocol`. There is no public `pong init`,
`pong attach`, or `pong connect` CLI, no `PONG_ENDPOINT` resolver, and no
localhost default selected by the Core.

`.pong/repository.json` is an internal compatibility/selector marker. It
currently describes repository format, schema, storage driver, and generation
selection. It is not a client bootstrap manifest and does not contain an
endpoint, bearer token, Agent identity, Workspace identity, or execution
state. The durable SQLite/CAS state remains inside the Core boundary.

## Existing Attach And Reconnect Semantics

Once the host has supplied the explicit repository and binding-root paths, the
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

The focused transport regression
`explicit_local_bootstrap_reconnects_same_repository_workspace_and_execution`
proves this explicit-path flow, including marker boundaries, repeat Agent
registration, Workspace lookup, Execution inspection, cold reopen, and the
absence of duplicate Workspace/Execution rows.

## Identity Boundaries

| Concern | Current authority | Bootstrap/authentication boundary |
| --- | --- | --- |
| Pong location | Host-supplied repository path and transport launch | Discovery, not authentication |
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

## Minimal Future Bootstrap Slice

The real gap is the local adapter/bootstrap layer, not the Core data model.
For a Runtime launched from a project such as `D:\\bs\\seekwd`, a future
offline-first bootstrap tool may provide a small non-secret local descriptor or
CLI resolution rule containing only:

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

Pong currently supports **explicit local bootstrap plus durable reconnect**.
It does not yet support **zero-knowledge discovery from an arbitrary project
directory**. That missing capability should be addressed by a future CLI/local
bootstrap slice, not by changing Protocol v1.0 or by moving Runtime behavior
into Workspace/Core.
