# M4-019 Windows Test Hazard Audit

**Baseline:** `7a12ed2b4dac2170f5dae64156c9bc58cc7e3b0b`
**Environment:** Windows NT 10.0.26200.0, rustc 1.95.0, Cargo 1.95.0.
**Status:** Windows default full regression PASS; direct-access stress
CHARACTERIZATION / unresolved `OPEN` code 2. No Model A support claim.

## Topology And Isolation

| Suite | Topology | Fixture and lifecycle |
| --- | --- | --- |
| `control_layer::three_provider_metadata_values_publish_to_isolated_workspaces` | Three **threads in one test process**, each independently opens the same Repository, then publishes into a distinct Workspace | One per-test `TempDir` Repository and one per-test Workspace parent; owner handle dropped before threads; each thread joined before directory cleanup. This is same-process independent-handle concurrency, **not** direct multi-process. |
| `local_concurrency::real_multi_process_open_*` | Holder/probe child processes with a common temporary Repository | Opt-in `direct-access-stress`; readiness/stop files, wait on normal exit, and a child guard that waits after emergency termination on parent unwind. |
| `local_concurrency::real_multi_process_writers_*` and `real_multi_process_workspace_publications_*` | Four/three OS child processes with independent `Repository::open` against one per-test temporary Repository | Opt-in `direct-access-stress`; every child is waited before cold reopen; failures now identify `OPEN`, `WRITE`, or `SNAPSHOT`. |
| `repository_access_policy` | Two child direct probes while one Core owner holds the lock | Each child waited, then owner dropped, cold reopen verifies the marker and absent Operation. This is a deterministic negative Model A boundary. |
| `core_ownership`, broker, remote/HTTP suites | One Core owner and clients, plus second-Core rejection | Temporary Repository, Workspace, credential and control roots; client/server processes are waited before successful fixture cleanup. Forced-exit tests wait before cold reopen. |

No fixed `D:\pong` Repository, lock file, SQLite file or Workspace path is
shared by these fixtures. Each uses `tempdir()`; only the deliberate writers
within a single test share their own temporary Repository. Cargo's independent
test binaries may run concurrently, but these specific fixtures have no
common namespace. The `control_layer` failing test joins its threads before
its temporary directories drop. The M4-018 full run also observed a separate
`agent_execution` process exit with `0xc0000005`; its isolated rerun passed,
and a causal link to direct Repository lifecycle is **NOT_PROVEN**.

## Open And Publication Phases

`Repository::open` first takes a process-refcounted shared Core-access fence
and a separate shared `repository.lock`, reads the marker, scans all `.pong`
owned bytes, opens SQLite, scans SQLite `-wal`, `-shm`, `-journal` and main
bytes before and after schema initialization, verifies CAS, scans `.pong`
again, then recovers unfinished Operations. Lock-file names themselves are
excluded from scanning. SQLite uses WAL and a 5-second busy timeout.
`scan_owned_directory` enumerates, then `symlink_metadata` and `fs::read`
each entry. `scan_database_bytes` treats `NotFound` on an optional sidecar
as absence but not a `NotFound` on a required file elsewhere. Startup
scanning while another handle changes SQLite sidecars or CAS staging is a
plausible race; it is **not yet a proven syscall-level attribution**.

The observed `control_layer` failure is `parallel open` before publication,
not a failed publication or corrupted Version. Existing reports observed
Windows raw OS codes 2 and 33 in this path. Code 33 may also arise during
filesystem reads of locked SQLite bytes; the explicit repository lock maps
`WouldBlock` and Windows 32/33 to `CONFLICT`, so a surfaced `PongError::Io`
was not classified by that lock mapper. Code 2 could be a transient
enumeration/sidecar race, but the exact I/O call is **NOT_PROVEN**. Code 5
could be ACL denial or another access condition; no global reclassification
is valid. Code 32 is a known sharing violation but is not a universal
success outcome. No error code alone proves fail-closed safety.

Repository fields retain Core-access and repository lock handles, CAS and
SQLite MetadataStore for the handle lifetime. The repository lock's `Drop`
calls `unlock` once; the File closes on its normal drop. The Core-access
registry removes its owned File when its last handle exits. No explicit
double-unlock or double-close was found in this audit. Field drop ordering
and OS handle release under abnormal process termination have not been
proved as the cause of a native access violation or heap corruption.

## Gate Principle

Supported Runtime topology is multiple clients through **one Core owner**;
second Core and direct opens must reject. Independent direct Repository
writers remain Model A `NOT_SUPPORTED`, but negative tests must still cold
reopen and establish valid Workspace/Snapshot/Version/Operation state.
Deterministic coordination of a single fixture's test phases is permitted;
serializing the entire test binary, reducing Cargo test concurrency,
whitelisting raw OS codes, deleting or ignoring tests is not.

## M4-019 Gate And Residual Risk

The prior `control_layer` test opened three independent handles at once.
That combined startup integrity scans with the behavior the test intended
to exercise: concurrent publication. The handles are now opened before a
barrier; three threads still publish simultaneously to separate Workspaces,
then all join before a cold reopen verifies the three Workspace heads,
Snapshot/Version linkage, completed Operations, and exactly one Version per
Workspace. This separates startup from publication without serializing the
publication itself or the test suite.

The default negative test creates a baseline Version and Operation, holds a
Core owner, and launches three real direct-access child writers. All must be
rejected at `OPEN` with a domain `Conflict`; after all children exit and the
owner drops, cold reopen checks the Workspace head and revision, Version,
Snapshot, Operation, Version count, and physical file content. The
unsupported race diagnostics remain compiled and runnable explicitly with
`cargo test --locked --features direct-access-stress --test local_concurrency`.
They are not ignored tests and do not run in the ordinary supported-path
release gate. One explicit opt-in run failed at `FAILED:OPEN:Io(Os { code: 2,
kind: NotFound })`; its children were waited and its cold-reopen checks ran
before reporting the unexpected result. The exact syscall and whether
Windows code 2 denotes a safe direct-access conflict are **NOT_PROVEN**.
Codes 5, 32, and 33 are not globally accepted either. No product behavior
or error mapping was changed to conceal these observations.

| Scenario | Windows result |
| --- | --- |
| Single Repository open; same-process independent handles with simultaneous publication | PASS in full regression |
| Single Core owner, multiple Runtime clients, second Core rejection | PASS in focused and full regressions |
| Direct writers against active Core owner; cold-reopen safety | PASS, three rejected child processes |
| Independent direct multi-process writers without Core owner | NOT_SUPPORTED; opt-in stress CHARACTERIZATION, code 2 failure |
| Core crash / Runtime crash recovery | PASS in existing regression |
| Native `0xc0000005` / `0xc0000374` causality | NOT_PROVEN; neither appeared in this full run |
| `cargo test --all --locked` | PASS, exit 0, default concurrency |

This single successful full run restores the measured gate, not a statistical
stability claim. Cross-test sharing, temp cleanup races, and native-crash
causality were not reproduced or conclusively excluded. Linux and macOS
parity remain NOT_PROVEN. Model A remains NOT_SUPPORTED.
