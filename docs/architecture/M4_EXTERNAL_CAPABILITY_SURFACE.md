# M4-017 External Capability Surface

**Status:** `PARTIAL / CAPABILITY INVENTORY FROZEN / AUTHORIZATION GATE OPEN`

**Baseline:** `0b17cc1920df6347466b573539e8ab58f4b8764f`
(`checkpoint: m4-remote-operability-failure-semantics`)

Core capability means that Pong can perform an action through an internal,
Core-owned API. External capability means that an authenticated Principal,
bound Agent, and Protocol v1.0 request can perform it. A public Rust method
does not grant an external Runtime authority. JSONL and HTTP carry the same
Protocol DTOs and do not add commands.

## Capability Matrix

`PASS` in a transport column means the existing v1.0 operation is carried,
not that every physical deployment or authorization policy is proven.
`PARTIAL` identifies a narrower exposed shape; `INTERNAL` is Core-owned but
absent from the wire; `NOT_SUPPORTED` is absent from Protocol v1.0.

| Capability | Pong Core | AgentControl | Protocol v1.0 | JSONL | HTTP | External Runtime |
| --- | --- | --- | --- | --- | --- | --- |
| Agent create/read | PASS | PASS | PASS | PASS | PASS | PASS, bound Agent registration |
| Task create/read | PASS | PASS | PASS | PASS | PASS | PARTIAL, registered-Agent read scope |
| Execution create/read/start/finish/failure | PASS | PASS | PASS | PASS | PASS | PASS, owner and revision guards |
| Operation start/finish/get/resolve/inspect | PASS | PASS | PASS | PASS | PASS | PASS, owner/request identity guards |
| Workspace create/read | PASS | PASS | PASS | PASS | PASS | PARTIAL, registered-Agent read scope |
| Workspace materialize from Version | PASS | PASS | PASS | PASS | PASS | PASS, Execution binding, lease, revision |
| Version publish/read/set current | PASS | PASS | PASS | PASS | PASS | PARTIAL, publish guarded; read scope broad |
| Snapshot publish/read | PASS | PARTIAL | PARTIAL | PARTIAL | PARTIAL | PARTIAL, publication and Version/inspection only; no `get_snapshot` |
| Checkpoint create/get/list | PASS | PASS | PASS | PASS | PASS | PARTIAL, creation bound; reads broad |
| Handoff create/get/list | PASS | PASS | PASS | PASS | PASS | PARTIAL, creation bound; reads broad |
| Resume from Checkpoint | PASS | PASS | PASS | PASS | PASS | PASS, Task/Version relation checked |
| Resume directly from Version | PASS | PASS | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | DEFERRED |
| Rollback to Version/Checkpoint/baseline | PASS | PASS | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | INTERNAL; external DEFERRED |
| Destination-only Snapshot restore | PASS | INTERNAL | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | INTERNAL |
| Restore from Version into Workspace | PASS | PASS | PARTIAL, `materialize_version` effect | PARTIAL | PARTIAL | PARTIAL, no independent `restore` authority |
| Workspace diff against Version | PASS | PASS | PASS | PASS | PASS | PARTIAL, read-only but registered-Agent scope |
| Snapshot/Version/Checkpoint diff as separate commands | PARTIAL | PARTIAL | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | DEFERRED |
| Lease acquire/renew/release | PASS | PASS | PASS | PASS | PASS | PASS, Execution binding and lease authority |
| Revision/CAS | PASS | PASS | PASS | PASS | PASS | PASS, explicit expected revisions |
| Operation result reconciliation | PASS | PASS | PASS | PASS | PASS | PASS, Agent/project request identity |
| Repository startup/journal recovery | PASS | INTERNAL | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | INTERNAL |
| Capability discovery | PARTIAL | INTERNAL | PASS, `hello` | PASS | PASS | PASS for advertised v1.0 set |
| Remote cancellation / SDK / MCP | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | NOT_SUPPORTED | DEFERRED |

Core and AgentControl inventory follows `src/control.rs`, `src/workspace.rs`,
`src/metadata.rs`, and `src/repository.rs`. Protocol inventory follows the
closed `ProtocolCall` enum, dispatch, and `hello` arrays in `src/protocol.rs`.
The JSONL/HTTP columns follow the shared `ProtocolDispatch` route. The complete
independent-process workflow and transport-equivalence test are in
`tests/real_remote_e2e.rs`. Core-only methods were not counted as wire tests.

## Capability Classes And Authority

| Class | Agent Runtime | Control Client | Recovery Client | Embedded Core |
| --- | --- | --- | --- | --- |
| OBSERVE | Registered Agent; Execution/Operation owner checks, other resource reads broad (`PARTIAL`) | No separate client role | No separate client role | Yes |
| EXECUTE | Own Execution, bound Workspace, lease/revision where required | No separate client role | No separate client role | Yes |
| CONTROL | Own Execution transitions and lease acquire/renew/release; no process kill | No elevated wire role | No elevated wire role | Yes |
| RECOVERY | `get_operation`, `resolve_operation`, inspect, checkpoint Resume; no rollback authority | No elevated wire role | No elevated wire role | Yes, including rollback/startup recovery |

The credential verifier binds Principal to an Agent grant, not to a project
membership or a privileged operator role. Protocol v1.0 verifies registered
Agent identity and checks Execution/Operation ownership and lease/revision on
mutations. `get_task`, `get_workspace`, `get_version`, `get_checkpoint`,
`get_handoff`, their lists, and `diff_workspace_version` do **not** require
resource ownership or project membership. A registered Agent can discover
another Agent's Workspace metadata by ID; diff can expose file names and
change metadata for a compatible Version. This is a real `PARTIAL` read-authorization
boundary, not permission inferred from write guards. No cross-project tenant
isolation is claimed. The current external endpoint must be limited to a
single trusted collaboration domain until a separately designed read policy
exists. Granting that policy would need a new authority model and is not
smuggled into frozen Protocol v1.0 here.

## Decisions

**Rollback: INTERNAL / external DEFERRED.** It is not a normal Runtime
continuation step. `AgentControl::rollback` calls the Core's local rollback
path, which requires a target Version/Checkpoint/Execution baseline, Task and
Execution scope, actor identity, an active Workspace lease, expected revision,
request/rollback identity, and a timestamp. It prepares a durable rollback
record before replacing physical state, then verifies and publishes the
Workspace Head and possibly Version Head, retaining history. A foreign source
may publish a target-local Snapshot and leave Version Head unchanged. The
record and request digest support exact replay; a prepared result may require
recovery, and reconnect/Core restart must inspect rather than infer success.
This is high-impact recovery authority despite history preservation. There is
no dedicated Protocol DTO, result query, or operator authorization grant.
Advertising or exposing it to all credential-bound Agents would be unsafe.
No rollback command is added in M4-017.

**Restore: INTERNAL.** `WorkspaceManager::restore_local` writes a verified
Snapshot to a caller-selected destination and records a durable Operation;
arbitrary destination selection is inappropriate for the external wire.
`AgentControl::restore_from_version` is currently an alias of
`materialize_from_version`, not a separate external authority. The guarded
target-Workspace materialization is already an EXECUTE command used for
handoff; it must not be advertised as general Restore.

**Diff: EXTERNAL OBSERVE / authorization PARTIAL.** v1.0 exposes read-only
`diff_workspace_version`. The Core compares a target physical tree to an
immutable compatible Version without changing lease, revision, Operation,
event, CAS, or Workspace Head. It does not expose arbitrary Snapshot,
Checkpoint, or Version-to-Version diff commands. The current read scope is
registered-Agent, not Workspace owner.

**Lease control: EXTERNAL CONTROL.** Runtime requests it explicitly because
Workspace publication/materialization require a lease authority; Core owns
epoch, expiration and exclusive grant. Acquire requires an owned Execution
bound to the Workspace; renewal/release require a valid matching token. It is
not a general lock-management or privileged recovery API.

**Recovery: SPLIT.** Agent Runtime may inspect/resolve its durable Operation
after disconnect and resume from an authorized Checkpoint. Core-owned
unfinished-journal recovery and rollback are INTERNAL. There is no special
Recovery Client role or remote cancellation path.

## Minimum External Workflow

The existing `real_process_full_checkpoint_handoff_resume_lifecycle_survives_restart`
test drives independent HTTP client/server processes through Agent registration,
Task, Execution A, Workspace A, Operation, Snapshot/Version, Checkpoint,
interruption, Resume Execution B, Handoff, target Workspace materialization,
continuation Version and Checkpoint, Core restart, and durable queries.
`provider_neutral_handoff_resume_materialization_survives_cold_reopen` verifies
the same domain route in the protocol suite. Neither workflow requires direct
Repository API from the Runtime. Rollback and destination-only Restore are not
required to complete it.

## Discovery And Compatibility

`hello` advertises exactly 20 commands and 14 queries at version `1.0`, no
rollback, restore, TLS, SDK, MCP, cancellation, or Core recovery. The separate
`RemoteAccessBoundary::capabilities` describes session/reauthentication
semantics, not extra Protocol commands. Old-style v1.0 hello without optional
`operation_id` succeeds. Unknown command and unknown fields fail strict
envelope decoding with stable `VALIDATION_ERROR` through transport; version
`2.0` returns `UNSUPPORTED_VERSION`. Unknown commands are not falsely reported
as supported nor silently ignored. This is the frozen v1.0 behavior; a new
`UNKNOWN_COMMAND` code would be a protocol change.

Any future external capability requires a command, truthful `hello` entry,
authorization rule, and regression tests. MCP, if later implemented, can only
adapt the same external protocol and cannot bypass its limits. SDK remains
deferred until the authorization boundary is closed.

## Gate

Inventory, truthful discovery, minimal workflow, and HTTP/JSONL equivalence
are proven. The authorization gate is **PARTIAL** for cross-resource OBSERVE,
so the conditional `m4-external-capability-surface` checkpoint is **not**
created. Production deployment, TLS, Linux/macOS, secret manager, slow-client
deadline and `tiny_http` parser queue remain `NOT_PROVEN`.
