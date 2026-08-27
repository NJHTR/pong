# M1 Native CI Execution Required

**Observed on 2026-08-27:** the local checkout contains
`.github/workflows/m1-release-evidence.yml`, but the GitHub repository
`NJHTR/pong` reports zero workflows, zero workflow runs, and no releases through
its unauthenticated API. The current `dev` branch is not published at the
workflow-bearing commit. No native Linux or macOS result is claimed.

```text
PUSH_REQUIRED = true
```

## Required external steps

1. Review the dirty-worktree changes and create a normal review commit that
   includes the workflow and its evidence-document updates.
2. Push that commit to a branch visible to `NJHTR/pong` (do not create a
   release tag yet).
3. Open **Actions -> M1 release evidence** and run it with **Run workflow**;
   a pull request touching the configured paths also starts it.
4. Wait for both matrix jobs to finish. A green workflow is required; a
   failed job is `FAIL`, not `BLOCKED`.
5. Download `m1-linux-native-evidence` and
   `m1-macos-native-evidence`. Each artifact must contain
   `platform/platform-metadata.json`, `artifact-manifest.json`,
   `platform/*`, build/test logs, focused M1 logs including `cold-reopen.log`,
   per-command `.exit` files, and `SHA256SUMS`.
6. Return the workflow run ID, commit SHA, job conclusions, artifact names,
   and artifact SHA-256 values for review. The artifacts must then be retained
   under `artifacts/m1-release-evidence/platform/` and referenced from the
   release matrix before either native row can become `PASS`.

## What the workflow verifies

- Ubuntu `24.04` and macOS `14` hosted runners;
- stable Rust and the declared Rust `1.78` MSRV, each with an independent
  `CARGO_TARGET_DIR`;
- locked dependency quality gates (`fmt`, `check`, `test`, and clippy);
- PT-13/FI-10, migration, recovery, compatibility, and explicit
  `cold_reopen` test evidence;
- runner OS/version, kernel, architecture, actual workspace and test-repository
  filesystem (tests use a workspace-local `TMPDIR`), Rust/Cargo versions,
  commit SHA, workflow run ID, exact commands, exit codes, and hashes.

Until a successful run and retained artifacts exist, the authoritative state
remains:

```text
Native Linux = BLOCKED
Native macOS = BLOCKED
M1 Release Gate = NOT PASSED
```
