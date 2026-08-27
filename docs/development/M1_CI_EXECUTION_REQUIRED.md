# M1 Native CI Execution Required

**Updated on 2026-08-27:** the first manual dispatch completed as workflow run
`33074865773` on commit `1212930d5d4faab9ca6b66cb8745475ad9a6de46`. Both native
artifacts were downloaded and retained under
`artifacts/m1-platform-runs/github-actions-run-33074865773/`, but both matrix
jobs failed their full quality gates. Linux found evidence-log line-ending hash
drift; macOS found a platform-inapplicable host-resource test compilation
failure. The corrective commit must be pushed and dispatched again.

```text
PUSH_REQUIRED = true
RERUN_REQUIRED = true
```

## Required external steps

1. Sign in to GitHub in the browser session that opens
   `https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml`.
2. After the corrective commit is pushed, open **Actions -> M1 release
   evidence** and run it with **Run workflow**; a pull request touching the
   configured paths also starts it.
3. Wait for both matrix jobs to finish. A green workflow is required; a
   failed job is `FAIL`, not `BLOCKED`.
4. Download `m1-linux-native-evidence` and
   `m1-macos-native-evidence`. Each artifact must contain
   `platform/platform-metadata.json`, `artifact-manifest.json`,
   `platform/*`, build/test logs, focused M1 logs including `cold-reopen.log`,
   per-command `.exit` files, and `SHA256SUMS`.
5. Return the workflow run ID, commit SHA, job conclusions, artifact names,
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

Until a successful rerun and retained artifacts exist, the authoritative state
remains:

```text
Native Linux = FAIL (run 33074865773); rerun required
Native macOS = FAIL (run 33074865773); rerun required
M1 Release Gate = NOT PASSED
```
