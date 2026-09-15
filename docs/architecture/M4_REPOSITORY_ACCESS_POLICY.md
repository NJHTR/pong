# M4-011 Repository Access Policy

**Status:** `PASS / POLICY-FROZEN`

Pong has one supported external-Runtime state authority: a single local Core
owner. This policy distinguishes state ownership from Rust library embedding,
transport, and unsupported direct multi-process access.

## Access Modes

| Mode | Path | Status |
| --- | --- | --- |
| A: Core owned | Runtime -> External Agent Protocol -> AgentControl -> Core -> Repository | SUPPORTED |
| B: Embedded | Rust application/test/offline tool -> Repository API | SUPPORTED INTERNAL / EMBEDDED |
| C: Direct multi-process external | Runtime process -> Repository, bypassing Core | NOT_SUPPORTED |

Mode A is the normal runtime architecture. The Core owns `Repository`,
`MetadataStore`/SQLite, CAS, Workspace filesystem mutation, Operation recovery,
and the repository owner fence. Agent identity, Execution identity, Workspace
lease, Operation identity, and Runtime session remain separate.

Mode B remains a public Rust integration surface. It supports library
embedding, tests, and offline migration or maintenance while no Core owner is
active. Pong does not impose a Core-only Rust type system. An embedded caller
is responsible for being the local state authority and for using lease,
revision, CAS, and transaction contracts correctly.

Mode C is not a Runtime feature. SQLite support for multiple connections does
not make multiple independent Runtime processes valid repository authorities.
Pong retains direct-process diagnostics as negative safety tests, not as a
promise that all direct writers will make progress.

## Production Boundary Audit

The production binary `pong-agent-protocol` opens storage only through
`Repository::open_as_core_owner`. It dispatches structured protocol v1.0
requests through `ExternalAgentProtocol` and `AgentControl`. No production
External Runtime entry point directly opens `Repository` or `MetadataStore`.

Direct `Repository::init/open` uses in the current checkout are library
implementation, integration tests, cold-reopen checks, and offline migration
or maintenance paths. Direct `MetadataStore` construction is used by primitive
and compatibility tests and remains below the external Runtime boundary.

The test-only M4-010 filesystem broker multiplexes real Runtime child processes
into one Core owner. It proves the Core contract but is not a production
multi-client transport. JSONL stdio remains one attached stream.

## Ownership and Concurrency

The owner fence and domain concurrency controls solve different problems:

- `core-owner.lock` selects the one live Core authority.
- `repository.lock` protects repository namespace and migration work.
- Workspace lease, epoch, and expiry identify the Agent write authority.
- revision and CAS reject stale logical writes.
- Operation request identity provides durable retry behavior.

The Core owner fence rejects a second Core and new embedded/direct opens with
the stable domain code `CONFLICT`. Runtime disconnect does not release durable
Agent, Execution, Operation, or Workspace state. Core termination releases the
OS lock; replacement Core recovery uses durable state.

## Unsupported Access Safety

Unsupported does not permit corruption. Direct multi-process diagnostics must
accept only:

1. a successful operation whose complete durable result is correct; or
2. an identified fail-closed access conflict with no partial publication.

After contention, cold reopen must preserve repository integrity, existing
Workspace heads, and completed durable results. Silent overwrite, partial
publication, invalid metadata, or an invalid Workspace head is always a bug.

`unsupported_direct_multi_process_access_fails_closed` verifies two real
processes cannot bypass an active Core owner. Both receive `CONFLICT`; after
owner shutdown the Repository cold-reopens unchanged. The existing M4-008
direct-writer tests remain active to diagnose Mode C when no Core owner exists.

## OS Error Classification

| Platform/error | Classification | Evidence |
| --- | --- | --- |
| Owner-lock `WouldBlock` | Repository access conflict | Stable `CONFLICT` mapping |
| Windows 32 (`ERROR_SHARING_VIOLATION`) | Sharing conflict | OS-defined and observed |
| Windows 33 (`ERROR_LOCK_VIOLATION`) | Lock conflict | OS-defined and observed |
| Windows 2 (`ERROR_FILE_NOT_FOUND`) during concurrent scan | Transient namespace race observation | Context-dependent; not a global conflict code |
| Windows 5 (`ERROR_ACCESS_DENIED`) | Ambiguous ACL denial or sharing/access race | `NOT_PROVEN`; remains `PERMISSION_DENIED` at protected boundaries |
| Linux/macOS lock errors | Expected to map through `WouldBlock` | Runtime parity `NOT_PROVEN` |

Code `5` is not globally converted to `DIRECT_ACCESS_CONFLICT`. Pong has native
ACL evidence where the same code is a real permission denial. The M4-010 full
run observed code `5` in an unsupported Mode C writer, but lacked path/phase
evidence because the prior diagnostic combined open and snapshot. The updated
diagnostic now records `OPEN` versus `SNAPSHOT` without changing its accepted
outcomes and validates cold-reopen Workspace heads before reporting an
unclassified error. Thirty focused reruns observed only successful writes,
open-time code `33`, and one open-time code `2`; they did not reproduce code
`5`.

## Release Gates

The Core release gate covers the supported external mode:

- single Core owner and second-Core rejection;
- external Runtime requests through Core;
- Runtime crash isolation and Operation resolution;
- Core forced-exit recovery and replacement ownership;
- reconnect and durable state reconstruction;
- lease, revision, CAS, and idempotent Operation behavior;
- no network, HTTP, MCP, or provider dependency.

Mode C availability is not a Core release feature. Its diagnostics remain
required negative safety evidence and must never be ignored or weakened.
Windows direct multi-process access remains `PARTIAL / IMPLEMENTATION
LIMITATION`; an unclassified failure remains visible and can keep a development
checkpoint test-gated even though the supported Core path passes.

## Status Matrix

| Capability | Status |
| --- | --- |
| Core owned access | PASS on Windows |
| External Runtime via Core | PASS in focused real-process validation |
| Embedded direct access | SUPPORTED INTERNAL / EMBEDDED |
| Direct multi-process external access | NOT_SUPPORTED |
| Unsupported access fail-closed | PASS with active Core; PARTIAL direct-vs-direct |
| Windows direct access | PARTIAL / IMPLEMENTATION LIMITATION |
| Windows code 5 classification | NOT_PROVEN |
| Linux/macOS owner and direct-access parity | NOT_PROVEN |
| M4-010 release readiness | PASS under the Core-owned external Runtime policy |
| Remote transport and multi-Pong sync | DEFERRED |
