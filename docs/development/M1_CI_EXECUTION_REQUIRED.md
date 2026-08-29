# M1 Native CI Execution Required

**Run #5 success update (2026-08-28):** workflow run `33145714975` at commit
`6cb62fb455e92ab731a4bb5233856d10c1f1ce93` completed both native jobs
successfully. Linux and macOS artifacts are retained under
`artifacts/m1-platform-runs/github-actions-run-33145714975/` and indexed in
the release bundle. Every one of the 13 recorded commands per platform
returned exit code `0`, including full stable/MSRV tests and artifact
consistency. ZIP SHA-256 values are Linux
`C4880D66D2AC18A5F2E2EF1DA8C375A3689C4D11B9E26DCCC26A96B34819C58A` and macOS
`EC89DC1F4A28AC0834173184E8BFF5C08D0A0DA23803F0D057AF6BB48A49BA3A`.
The native executable evidence rows may now be marked `PASS`; M1 itself
remains `NOT PASSED` pending the separately released old reader, complete
fault/platform scope, performance budget, and named release-owner acceptance.

**Run #4 audit update (2026-08-28):** workflow run `33142438624` at commit
`e96131d9bb6d3799055401841d0cb710e4f497ff` produced complete Linux and macOS
artifacts. Only the two full `cargo test` commands per platform failed, both
at `artifact_consistency.rs:157` on the committed `build-metadata.json` hash:
the references described Windows CRLF bytes while the clean checkout contained
the canonical LF blob. Run #4 ZIP SHA-256 values are Linux
`DA903E4D7C6E11D3DD08BEE598883BE813E606305FA56ABA91D2154ACD0E6666` and macOS
`501832097AB33640CFE062A98293CC4A36C7B83C8FCAF3918253D89F2716AD34`.
The release checksum/reference records are corrected in the current worktree;
push them and dispatch once more. Run #4 remains `FAIL` evidence.

**Run #3 audit update (2026-08-27):** workflow run `33085292318` at commit
`60913c0179731d9fed1a052f9a190c8fa4f5a56e` produced complete Linux and macOS
artifacts. Stable/MSRV fmt/check/clippy and focused M1 suites passed on both;
both full test commands exited `101` in `artifact_consistency` because the
run consumed committed LF evidence while the references still described prior
Windows working-tree bytes. Linux ZIP SHA-256 is
`D8C255461BA1A64C712B55D79837F59B4B15E63A7D2A2341F4B86B54BCA57E0E`; macOS
ZIP SHA-256 is
`3C16C855381479C30B86BF6E3061818306F23DC55C7F9E11240178000A4A5692`.
The macOS runner's filesystem metadata is `unknown`, so it is not recorded as
APFS evidence. Run #3 remains `FAIL`; a fresh dispatch after the repository
byte-boundary correction is required.

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
and is pushed to `origin/dev`. This close-out additionally broadens the
`.gitattributes` byte boundary to all `artifacts/**` and points PT-13/FI-10 at
the release-log copy; those changes are committed and pushed in
`258a0c9cccfe11e994a6032a106644b4dcf90804`. A fresh manual dispatch is still
required.

```text
PUSH_REQUIRED = true
RERUN_REQUIRED = true
PENDING_HEAD = current worktree (checksum/reference correction plus Run #4 retention)
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

The successful run `33145714975` and its retained artifacts are now the
authoritative native executable evidence:

```text
Native Linux = PASS (executable evidence; run 33145714975; ext4)
Native macOS = PASS (executable evidence; run 33145714975; filesystem unknown)
M1 Release Gate = NOT PASSED
```
