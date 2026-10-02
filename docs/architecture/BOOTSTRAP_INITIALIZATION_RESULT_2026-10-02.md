# Pong Bootstrap Initialization Result

**Date:** `2026-10-02`

**Pong HEAD before this slice:** `19b20f0`

**Target Repository:** `C:\Users\NJHTR\IdeaProjects\easyCode`

**Slice:** Pong Bootstrap / Runtime Initialization

## Repository and Bootstrap API

- `Repository::init` remains the single Repository initializer and does not
  require Git metadata or a clean Git working tree.
- `BootstrapMetadata::write` remains the writer for the existing v1 descriptor.
- `bootstrap::initialize` is the formal composition entry point added in this
  slice. It calls `Repository::init`, creates `.pong/workspaces`, writes the
  descriptor only when absent, and validates the result through `discover`.
- `pong-bootstrap initialize <repository-root>` is the executable entry point.
- The generated descriptor uses the existing schema with
  `workspace_root: ".pong/workspaces"`, `workspace_id: null`, and
  `core_endpoint: null`. No repository/workspace durable identity field was
  added.

## Tests

Command:

```text
cargo test --locked --test bootstrap_initialization --test agent_bootstrap --test external_agent_protocol_transport -- --test-threads=1
```

Result: `10 passed, 0 failed`.

Covered behavior includes fresh and existing directories, non-Git directories,
user-file preservation, idempotent initialization, existing/incompatible
`.pong`, invalid paths, bootstrap parsing, protocol-process consumption, and
existing bootstrap/transport reconnect regressions.

Quality commands:

```text
cargo fmt --all -- --check        PASS
cargo check --all-targets --locked PASS
cargo clippy --all-targets --all-features --locked -- -D warnings PASS
git diff --check                  PASS
```

## Real easyCode Initialization

Precondition snapshot:

```text
Git repository: False
.pong before: False
Business files before: 79
```

Formal command:

```text
cargo run --locked --bin pong-bootstrap -- initialize C:\Users\NJHTR\IdeaProjects\easyCode
```

Actual result:

```text
status: ready
project_root: \\?\C:\Users\NJHTR\IdeaProjects\easyCode
repository_root: \\?\C:\Users\NJHTR\IdeaProjects\easyCode
workspace_root: \\?\C:\Users\NJHTR\IdeaProjects\easyCode\.pong\workspaces
bootstrap: \\?\C:\Users\NJHTR\IdeaProjects\easyCode\.pong\bootstrap.json
core_endpoint: null
```

Postcondition:

```text
Business files after: 79
Business file hash changes: 0
Pong-owned .pong files: 5
.pong/bootstrap.json: present
.pong/repository.json: present
```

The target was not a Git repository, and initialization succeeded without
touching business files. Only Pong-owned `.pong` metadata was created.

The immediate post-initialization comparison above was captured before the
Core smoke and final validation. A later read-only comparison observed
additional changes outside `.pong` (`README.md`,
`src/main/java/com/easycode/sandbox/WindowsAppContainerDemo.java`, and
`src/main/java/com/easycode/sandbox/WindowsJobObjectSandboxManager.java`, plus
generated `target` artifacts). Those changes were not written by the Pong
initialization or Core commands and were left untouched; because `easyCode` is
not a Git repository, their external provenance cannot be verified here.

## Core and Transport Smoke

Actual process command:

```text
target\debug\pong-agent-protocol.exe --project-root C:\Users\NJHTR\IdeaProjects\easyCode
```

Actual runtime observation:

```text
Core process: started with PID 26068
Request: operation=hello, protocol_version=1.0
Response: status=ok, result.kind=hello, protocol_versions=["1.0"]
Exit code: 0
stderr: empty
```

This proves bootstrap parsing, Repository Core ownership, and JSON Lines
transport reachability. The process was shut down by closing stdin; no Core
process remains running.

## register_agent

**Status:** `BLOCKED / RUNTIME IDENTITY`

The existing Protocol v1.0 implementation requires the Runtime to present a
real `agent_id` in `register_agent`; it does not allocate one from an empty
request. No authenticated Provider Runtime was running in this bring-up, so no
legitimate Agent ID, session, or external incarnation existed to submit. No
durable ID was fabricated and no invalid registration request was sent.

```text
Agent ID: n/a
Session: n/a
Incarnation: n/a
```

## Boundaries

- Protocol v1.0: unchanged.
- Core durable schema: unchanged.
- `REAL_MULTI_AGENT_E2E.md`: unchanged.
- `C:\Users\NJHTR\IdeaProjects\easyCode` business code: untouched by Pong;
  later external changes were observed and preserved.
- `D:\bs\seekwd`: not touched.
- Mock providers: not started.
- Scenario A-F: not entered.
