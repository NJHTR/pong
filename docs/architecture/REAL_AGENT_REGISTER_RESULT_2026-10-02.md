# Real Agent Register Result

**Date:** `2026-10-03`

**Baseline before this slice:** `a1b0277 docs: clarify provider identity mapping gap`

**Target Repository:** `C:\Users\NJHTR\IdeaProjects\easyCode`

**Scope:** Real Provider Runtime identity adapter and minimal `register_agent`
bring-up only. Scenario A-F was not entered.

## Environment

The previously completed bootstrap evidence remains authoritative:

- `Repository::init`: PASS
- `.pong/bootstrap.json`: PRESENT
- `pong-agent-protocol --project-root C:\Users\NJHTR\IdeaProjects\easyCode`:
  started successfully
- JSONL `hello`: PASS
- transport connectivity: PASS
- easyCode is not a Git repository; Pong initialization does not require Git
- Pong initialization created only Pong-owned `.pong` metadata and did not
  change easyCode business files

Provider binaries available during this probe:

```text
codex-cli 0.158.0-alpha.2.1
2.1.131 (Claude Code)
```

## Real Codex Runtime Probe

A real Codex process was started in a temporary system directory, separate
from easyCode, with this command shape:

```text
codex exec --dangerously-bypass-approvals-and-sandbox \
  --ephemeral --skip-git-repo-check --json \
  -C <temporary probe directory> \
  "Do not use any tools or modify files. Return exactly the word READY."
```

The process returned exit code `0` and emitted:

```json
{"type":"thread.started","thread_id":"01a0fbf3-bf08-7512-bbe0-6454744cbbb0"}
{"type":"item.completed","item":{"type":"agent_message","text":"READY"}}
{"type":"turn.completed"}
```

This is a real Codex runtime thread identity. It is not a Pong Agent ID,
Pong session, or Pong process incarnation.

## Identity Semantics From Existing Contract

The source-backed identity boundaries are:

| Value | Meaning | Creator/authority | Restart behavior |
| --- | --- | --- | --- |
| `agent_id` | Durable logical actor identity | Runtime adapter/credential-backed caller; Pong accepts and persists the asserted opaque value | Stable when the same logical identity is reused |
| `session_id` | Ephemeral transport or operation correlation value | Transport/runtime boundary; HTTP creates connection sessions, while JSONL has no durable session handshake | Changes on a new connection/process; not Agent identity |
| incarnation | Process/runtime generation observation | Not represented as a Protocol v1.0 durable field | Must remain outside Core until a lifecycle contract defines it |
| Codex `thread_id` | Codex runtime thread/run identity | Codex CLI | New thread observed on each independent probe; no evidence that it is a stable logical Agent identity |

`src/metadata.rs` stores `AgentIdentity` as `agent_id`, `provider`,
`display_name`, and `created_at`; the local adapter supplies the asserted
Agent ID without changing Core.
`register_agent` command requires an asserted `caller_agent_id` equal to the
payload `agent_id`. Existing architecture documentation explicitly says that
framework adapters translate native run IDs into Pong sessions and that
connections, PIDs, executable names, and provider sessions are not Agent
identity. Protocol v1.0 has no `incarnation` request or response field.

Therefore the current mapping is:

```text
Codex thread_id -> adapter-owned provider-run metadata only
Runtime identity file -> Pong agent_id
Adapter instance -> ephemeral session
incarnation -> NOT_IN_PROTOCOL_V1
```

This is not a valid direct `thread_id -> agent_id` mapping.

## Second Real Codex Probe

The current probe was run against `D:\pong` with no file tools permitted:

```text
codex exec --dangerously-bypass-approvals-and-sandbox \
  --ephemeral --skip-git-repo-check --json \
  -C D:\pong \
  "Do not use any tools or modify files. Return exactly the word READY."
```

Actual provider result:

```text
provider: Codex
version: codex-cli 0.158.0-alpha.2.1
exit_code: 0
thread_id: 01a0fc01-5235-78a2-afa4-22080dfcb63f
agent_message: READY
```

The JSON event stream contained `thread.started`, `item.completed`, and
`turn.completed`; it contained no provider session ID, logical Agent ID, or
incarnation field. The process PID was not treated as an identity because it
is an ephemeral process observation.

## Runtime Identity Adapter

The formal adapter persists one logical Runtime profile at:

```text
<repository>/.pong/runtime-identities/<provider-profile>.json
```

It creates the opaque `agent_id` once, recovers it on restart, and creates a
new ephemeral adapter `session_id` for each Runtime instance. Codex
`thread_id` is kept as provider-run metadata and is not used as `agent_id`.
The adapter sends stable provider metadata to `register_agent`; a changing
thread ID is not durable Agent metadata.

The identity contract is documented in
`docs/architecture/RUNTIME_AGENT_IDENTITY.md`.

## `register_agent`

**Status:** `PASS`

Protocol v1.0 requires the caller to provide an asserted `agent_id` in the
`register_agent` request. The local Runtime Identity Adapter now supplies a
stable logical identity without mapping the provider thread to that identity.
The request's provider metadata remains stable across registration replay.

```text
agent_id: agent:local:8ec1e657e9f7dd4ee7fcd4526a74d071505ec968b84343f7abb0235ecad0fdb7
session: session:local:616eb111ebc6ec506d607ae6b8af64acf4f4222b3e41ea0b653b41c845fd8316
incarnation: NOT_IN_PROTOCOL_V1
provider_thread_id: 01a1008d-d881-7380-ac91-02c82053cb02
latest_provider_thread_id: 01a1008d-d881-7380-ac91-02c82053cb02
```

The `agent_id` above was created by the formal local adapter and recovered
from its Pong-owned identity file; it was not supplied by hand and was not
derived from the Codex thread ID.

The resulting acceptance states are:

```text
Provider Runtime: PASS
Provider identity: PASS (Codex thread_id)
Identity mapping: PASS (adapter-created logical agent_id; thread_id separate)
Pong Core: PASS
Transport: PASS
register_agent: PASS
Agent inspection: PASS
Restart identity test: PASS
```

The real harness registered and inspected the Agent, shut down the protocol
process, reopened the same identity file, and registered/inspected again.
The logical `agent_id` remained stable while the adapter session changed.

```text
first_agent_id: agent:local:8ec1e657e9f7dd4ee7fcd4526a74d071505ec968b84343f7abb0235ecad0fdb7
second_agent_id: agent:local:8ec1e657e9f7dd4ee7fcd4526a74d071505ec968b84343f7abb0235ecad0fdb7
first_session: session:local:616eb111ebc6ec506d607ae6b8af64acf4f4222b3e41ea0b653b41c845fd8316
second_session: session:local:b03cf0edc6f55f7a06f87a3da36bb3d292f2030cbb2edfa15640529ae5771730
```

`first_agent_id == second_agent_id` and `first_session != second_session`.

Observed provider warnings (plugin authentication/sync, unsupported
PowerShell shell snapshot, and a deprecated configuration warning) did not
prevent the probe from returning `READY`; they are not classified as Pong
Core failures.

## Boundary Verification

- Production Core code: unchanged.
- Protocol v1.0: unchanged.
- Core durable schema: unchanged.
- `C:\Users\NJHTR\IdeaProjects\easyCode` business files: untouched.
- `D:\bs\seekwd`: not inspected or touched.
- Mock provider: not started.
- Scenario A-F: not executed.
- No Operation, Execution, Version, Checkpoint, session, or incarnation ID
  was fabricated.

## Classification and Next Action

The identity bridge is now complete for the local Codex Runtime smoke. The
incarnation value remains `NOT_IN_PROTOCOL_V1`; no Protocol extension was
needed. Scenario A-F remains intentionally deferred to a later turn.
