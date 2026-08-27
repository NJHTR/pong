# M1 Native CI Execution Required

**Updated on 2026-08-27:** the first manual dispatch completed as workflow run
`33074865773` on commit `1212930d5d4faab9ca6b66cb8745475ad9a6de46`. Both native
artifacts were downloaded and retained under
`artifacts/m1-platform-runs/github-actions-run-33074865773/`, but both matrix
jobs failed their full quality gates. Linux found evidence-log line-ending hash
drift; macOS found a platform-inapplicable host-resource test compilation
failure. A second manual dispatch completed as run `33080915116` on commit
`8d28075d44f5458e866c94ee33b95b430f7959d6`. Linux again failed both full tests
in `artifact_consistency`; macOS failed the Rust 1.78 test path (clippy exited
zero) and its
downloaded artifact omitted platform metadata, the manifest, stable logs, and
stable exit records. Both ZIPs are retained under
`artifacts/m1-platform-runs/github-actions-run-33080915116/` with Linux SHA-256
`B3E457A9B936347E619C280B5DB5ACE06A9C7A7C206178193D85527BDF9E0B1E` and macOS
SHA-256 `01A1BC12E9DE8051F603EEDA3CC23A920419067F38503F38635856628083F16F`.
The portability correction was committed as `359abc306b554d592b532ebc182e543f97489043`,
the evidence metadata was rebound in `2310c822a6f5d3f942067e0ec7a322369f03df8a`,
and the bundle-wide byte-preservation fix is in `96a2f8c`. Workflow
capture-boundary hardening is included in `2cf136e99091d8d074d1a0e597f72f06bd2d52f7`
and is pushed to `origin/dev`. A fresh manual dispatch is still required.

```text
PUSH_REQUIRED = false
RERUN_REQUIRED = true
PUSHED_HEAD = 2cf136e99091d8d074d1a0e597f72f06bd2d52f7
```

## Required external steps

1. Sign in to GitHub in the browser session that opens
   `https://github.com/NJHTR/pong/actions/workflows/m1-release-evidence.yml`.
2. Open **Actions -> M1 release evidence** at the pushed `dev` head and run it
   with **Run workflow**; a pull request touching the configured paths also
   starts it.
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
Native Linux = FAIL (runs 33074865773, 33080915116); rerun required
Native macOS = FAIL (runs 33074865773, 33080915116); rerun required
M1 Release Gate = NOT PASSED
```
