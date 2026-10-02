# Real Agent Register Result

**Date:** `2026-10-02`

**Pong HEAD:** `4b58c6a feat: add Pong bootstrap initialization entrypoint`

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
```

No `register_agent` request was sent because the required Pong identity
fields did not exist. The provider thread ID was not transformed into a Pong
ID, and no UUID or other durable ID was fabricated.

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
