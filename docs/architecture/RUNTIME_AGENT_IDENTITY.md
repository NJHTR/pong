# Runtime Agent Identity

**Status:** Minimal local adapter implemented; Protocol v1.0 unchanged.

## Boundaries

Pong distinguishes four values:

| Value | Authority | Lifetime |
| --- | --- | --- |
| `agent_id` | The local Runtime Identity Adapter's durable profile | Stable across Runtime restart when the same identity file is reused |
| `session_id` | The Runtime/transport adapter | New for each adapter process or connection |
| Provider `thread_id` | The provider Runtime, such as Codex | One provider run/thread; metadata only |
| incarnation | No Protocol v1.0 field | Not represented by Pong Core in this slice |

`agent_id` is a logical actor identity, not a PID, executable name, transport
connection, or provider thread. The adapter creates it once in the
Pong-owned `.pong/runtime-identities/<provider-profile>.json` file and reuses
it on restart. A separate profile file represents a different logical Agent.
The generated value is opaque local identity metadata, not an authentication
credential.

Each adapter instance creates a new ephemeral `session_id`. JSONL
`register_agent` does not carry a session field; the adapter may correlate its
session outside the frozen domain request. HTTP transport sessions remain a
separate transport concern.

Codex `thread_id` is retained as provider-run metadata and evidence. It is
never converted to `agent_id`, and it is not placed in durable Agent metadata
because changing a provider thread must not conflict with replaying the same
logical Agent registration.

## Lifecycle

```text
first Runtime start
  -> create identity metadata once
  -> create ephemeral session
  -> register asserted agent_id with stable provider metadata

Runtime restart
  -> read the same identity metadata
  -> create a new session
  -> replay register_agent idempotently

provider restart/thread change
  -> keep agent_id
  -> keep the new thread_id outside Agent identity
```

The adapter does not create a new Core Agent entity on every process start.
Core remains authoritative for the durable Agent record and inspection.
Provider crash/recovery, handoff, and checkpoint semantics remain the existing
Execution/Checkpoint/Resume contracts; this slice does not enter Scenario
A-F.

## Protocol Boundary

Protocol v1.0 already accepts an asserted opaque `agent_id` and optional
provider metadata. It has no independent durable `incarnation` field and this
slice does not add one:

```text
incarnation = NOT_IN_PROTOCOL_V1
```

An implementation that needs a process-generation observation must keep it in
the Runtime/transport evidence layer until a separately approved lifecycle
contract defines its semantics.
