# M4-018 External Resource Authorization

**Scope:** External Agent Protocol v1.0 at `28fb4125885955ea50e18f9652eac2502ed88bba`.
**Decision:** One trusted collaboration domain, shared resource observation,
Agent-owned Execution/Operation control. This is not multi-tenant isolation.

## Visibility Classes

| Class | Wire surface | Authority |
| --- | --- | --- |
| Discovery | `hello`, registered Agent identity | `hello` is global support discovery, not a grant; other queries require a registered asserted Agent |
| Shared collaboration observation | Task, Workspace inspection (including head, revision, status, environment, branch and active lease authority), Version, Checkpoint, Handoff and lists, Workspace/Version diff | Any registered Agent in this Core's trusted collaboration domain, by ID; no project membership or Workspace owner exists |
| Agent-scoped state | Execution get/inspect, Operation get/resolve | Owning Agent; `resolve_operation` also matches project and request identity |
| Bound control | Execution transitions, Operation lifecycle, Workspace lease, publication, checkpoint, handoff, materialization | Own Execution and its Workspace binding; active lease and expected revision where specified |
| Recovery | Checkpoint Resume, Operation reconciliation; rollback, arbitrary-destination Restore and Repository recovery | Resume creates a new caller-owned Execution under existing Task/Version/Checkpoint scope. Rollback/Restore/startup recovery are Core-owned and absent from v1.0 |

`Workspace` does **not** have a durable owner Agent. `create_workspace` does
not record a creator. A registered Agent can create a Task in a project and
bind its own Execution to a compatible Workspace. Project ID equality and a
self-created Execution are not membership credentials. Consequently the
Workspace and Version read surface is intentionally shared *within the
credential issuer's single trusted domain*. It must not be presented as
private per-Agent data or exposed to mutually untrusted tenants. A separate
grant model would require a separately approved design and durable authority;
it cannot be fabricated from an existing lease or revision.

## Authorization Matrix

`Same` and `Other` refer to the Agent owning a target Execution, not to a
nonexistent Workspace owner. `Other` means another registered Agent in the
same trusted collaboration domain. There is no privileged control Principal
or remote recovery role in v1.0.

| Resource / operation | Same | Other | Rule / boundary |
| --- | --- | --- | --- |
| Agent register | Own asserted ID | Own asserted ID | HTTP Principal must grant asserted Agent; JSONL trusts its local caller boundary |
| Agent read | Shared | Shared | Registered Agent identity/provider metadata |
| Task create | Allowed | Allowed | Registered Agent; no persisted Task creator or project membership |
| Task read | Shared | Shared | Goal/context/state are visible; not just Task ID |
| Execution create | Own new record | Own new record | Registered Agent, Task/Workspace/Version project bindings; no Workspace ownership inference |
| Execution get/inspect | Allowed | `FORBIDDEN` | Agent owner; inspection includes associated Task, Workspace, lease, Versions, Checkpoints, Handoffs, Resume and Operations |
| Execution transition / Operation start/finish | Allowed | `FORBIDDEN` | Agent owner, lifecycle/revision/request/Operation identity |
| Operation get/resolve | Allowed | `FORBIDDEN` or `NOT_FOUND` for caller-scoped resolve | Owner; resolve scoped by caller, project and request |
| Workspace create | Allowed | Allowed | Registered Agent and safe binding reference; no owner assignment |
| Workspace inspection | Shared | Shared | Full logical head/version/status/revision and active lease authority; no driver/locator |
| Workspace lease acquire | Bound Execution | Foreign Execution `FORBIDDEN` | Own Execution bound to target; existing foreign active lease conflicts even for own bound Execution |
| Lease renew/release | Valid token | `LEASE_CONFLICT` | Matching Agent/epoch/expiry; lease is **not** a read ACL |
| Snapshot publish / Version publish | Bound Execution | Foreign Execution `FORBIDDEN` | Lease, revision, project/environment and Operation identity; no standalone `get_snapshot` |
| Version get | Shared | Shared | Version/Snapshot IDs and relationship metadata; no manifest/blob contents |
| Set current Version | Bound Execution | Foreign Execution `FORBIDDEN` | Own Execution, compatible Workspace/Version, lease and dual revisions |
| Checkpoint get/list | Shared | Shared | Relation, reason and actor metadata; no blob contents |
| Checkpoint create | Bound Execution | Foreign Execution `FORBIDDEN` | Owned Execution/Workspace and compatible Version |
| Handoff get/list | Shared | Shared | Source/target relation and actor metadata |
| Handoff create | Source Execution owner | Foreign source `FORBIDDEN` | Source owner and valid Task/target/Version/Checkpoint relation |
| Checkpoint Resume | New caller-owned Execution | New caller-owned Execution | Existing Task/Checkpoint/Version scope; cross-Agent handoff is intentional |
| Workspace/Version diff | Shared | Shared | Relative paths, change types, hashes, sizes, file types; no file contents or absolute locator |
| Version materialize | Bound target Execution | Foreign target `FORBIDDEN` | Own target Execution, target lease/revision, compatible source Version |
| Rollback / arbitrary Restore / delete | No external command | No external command | Internal or deferred; not advertised by `hello` |

`get_workspace` is **detailed shared observation**, not merely public identity
metadata. A foreign Agent can see an active lease token's fields but cannot
present them as its own Agent because authorization checks the asserted caller
against lease Agent. A foreign Agent cannot operate another Agent's Execution.
After its own compatible Execution is created, it can compete for a free
Workspace lease: this is collaborative Workspace control, not per-Agent
Workspace privacy. The lease serializes mutation; revision/CAS prevents stale
writes. Neither is used as a read ACL.

## Path And Information Boundary

`WorkspaceResource` omits the persisted `driver` and absolute `locator`.
Version and Snapshot wire resources expose identifiers/counts but not the
manifest, CAS blobs or physical paths. Diff entries contain **relative file
names** and hashes/sizes/types; these are potentially sensitive collaboration
metadata and are deliberately visible to every registered Agent in the domain.
They must not be treated as public or tenant-safe. Protocol errors map storage
errors to stable codes/messages rather than returning SQLite or OS paths.
Caller-supplied opaque fields (for example Task goal, branch reference, reason
and entity ID in error details) are still caller-controlled text and are not
promised to be secret or path-free.

## Transport And Discovery

HTTP authenticates an opaque bearer credential and binds its Principal to
the asserted Agent before protocol dispatch. JSONL is an attached local
stdin/stdout transport under its host's OS trust boundary and has no HTTP
Principal or independent multi-client credential verifier. For the same
asserted registered Agent, both execute the same Protocol authorization.
`hello` lists the globally supported v1.0 commands/queries even before
registration: it is not a per-Principal capability grant. Rollback, Restore,
MCP and SDK are not offered. No transport can add resource authority to Core.

Tests: `tests/resource_authorization.rs` uses temporary Repository and
Workspace fixtures and real Protocol dispatch. The cross-Agent scenario in
`tests/real_remote_e2e.rs` additionally launches independent HTTP server,
HTTP client and JSONL processes over loopback TCP/OS pipes, comparing protocol
outcomes without conflating HTTP status with domain authorization. Existing
real remote lifecycle, restart and transport suites remain in force.

**Deployment prerequisite:** all credentials granting access to one Core
must represent Agents allowed to observe all shared Task/Workspace/Version/
Checkpoint/Handoff/diff metadata in that Core. Do not use this instance as a
boundary between mutually untrusted projects or tenants. TLS, public Internet
deployment, Linux/macOS parity, production secret-manager integration and
slow-client protection remain separately `NOT_PROVEN`; MCP and SDK deferred.

## M4-018 Gate

The resource visibility/read/control contract and focused Windows loopback
process tests pass in the stated trusted-domain model. Full regression is
`PARTIAL`: one run ended with `STATUS_ACCESS_VIOLATION` after the existing
`agent_execution` target's ordinary cases; an isolated rerun passed. A second
full run failed the unchanged direct-handle `control_layer` concurrency test
with Windows raw OS code 33; two isolated retries failed with codes 2 and 33.
Those are retained as distinct observations, not asserted to be M4-018
causation or accepted as a passing release gate. The conditional
`m4-resource-authorization-closure` checkpoint is **not created**.
