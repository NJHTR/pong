# Real Agent Register Result

**Date:** `2026-10-02`

**Pong HEAD at this update:** `a1e503a docs: record real agent register blocker`

**Target Repository:** `C:\Users\NJHTR\IdeaProjects\easyCode`

**Scope:** Real Provider Runtime bring-up through `register_agent` only.
Scenario A-F was not entered.

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
`display_name`, and `created_at`; it does not generate an Agent ID. The
`register_agent` command requires an asserted `caller_agent_id` equal to the
payload `agent_id`. Existing architecture documentation explicitly says that
framework adapters translate native run IDs into Pong sessions and that
connections, PIDs, executable names, and provider sessions are not Agent
identity. Protocol v1.0 has no `incarnation` request or response field.

Therefore the current mapping is:

```text
Codex thread_id -> provider metadata / external run reference only
Pong agent_id, session, incarnation -> no formal Runtime Adapter source
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

## `register_agent`

**Status:** `BLOCKED / RUNTIME_INTEGRATION_GAP`

Protocol v1.0 requires the caller to provide a real `agent_id` in the
`register_agent` request. The current Pong implementation has no Provider
Runtime abstraction or Codex/Claude bridge that maps a provider thread to a
Pong Agent identity, session, and incarnation. Pong does not allocate these
values, and Protocol v1.0 does not define a process-incarnation exchange.

```text
agent_id: n/a
session: n/a
incarnation: n/a
provider_thread_id: 01a0fbf3-bf08-7512-bbe0-6454744cbbb0
latest_provider_thread_id: 01a0fc01-5235-78a2-afa4-22080dfcb63f
```

No `register_agent` request was sent because the required Pong identity
fields did not exist. The provider thread ID was not transformed into a Pong
ID, and no UUID or other durable ID was fabricated.

The resulting acceptance states are:

```text
Provider Runtime: PASS
Provider identity: PASS (Codex thread_id only)
Identity mapping: BLOCKED
Pong Core: PASS
Transport: PASS
register_agent: BLOCKED
Agent inspection: BLOCKED (no registered Agent)
Restart identity test: BLOCKED (no first successful registration)
```

The requested Runtime A -> shutdown -> Runtime A restarted comparison was
not run as a claimed success path. Without a valid first registration there
is no legitimate pair of Pong identities to compare, and the Codex CLI probe
does not expose a stable logical Agent identity that could be reused.

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

This is a `RUNTIME_INTEGRATION_GAP`: the real provider can start and expose
its own thread ID, but Pong has no approved bridge that supplies the identity
contract required by `register_agent`. A future explicitly approved runtime
integration decision is required before retrying registration. Until then,
do not derive IDs, alter Protocol v1.0, or enter Scenario A-F.
