# Host Resource Fault Evidence

`host_resource_faults.rs` contains opt-in tests for the real host failure
boundary in FI-13 and FI-14. The tests are marked `ignored` in the ordinary
quality gate because they deliberately change an ACL or fill a disposable
filesystem. They do not arm Pong's synthetic failpoints. Every case takes an
OS-level lock at the parent scratch directory and uses a unique child
directory, so concurrent test processes serialize safely and a killed process
does not leave a held lock.

## Windows ACL (FI-13)

Run from an unelevated PowerShell session on an NTFS volume. The directory
named by `PONG_HOST_FAULT_ROOT` must be dedicated; before a run it may contain
only the harness-owned `.pong-host-fault.lock`. The test creates and removes a
child repository there.

```powershell
New-Item -ItemType Directory -Force D:\pong\.host-fault-run | Out-Null
$env:PONG_HOST_FAULT_ROOT = 'D:\pong\.host-fault-run'
cargo test --locked --test host_resource_faults `
  fi_13_real_windows_acl_read_revocation_fails_closed_and_recovers `
  -- --ignored --nocapture
cmd /d /c "rmdir /s /q D:\pong\.host-fault-run"
```

The test applies an explicit deny ACE for the current `whoami` identity to
the active metadata file, verifies the kernel returns `ERROR_ACCESS_DENIED`
(raw error 5), and requires Pong to return `PERMISSION_DENIED` or
`RECOVERY_REQUIRED`. It removes the ACE before the final cold reopen and
checks that the previously committed ref and CAS object remain readable.

## Linux disposable `tmpfs` (FI-14)

Docker Desktop is used only to provide a bounded Linux `tmpfs`; no host data
volume is exhausted. Each invocation gets a fresh 64 MiB `tmpfs`. The test
refuses any filesystem that is not `tmpfs` or exceeds 64 MiB. The harness lock
serializes multiple invocations that happen to share one scratch root.

```powershell
docker run --rm --init `
  --mount type=bind,src=D:\pong,dst=/workspace `
  --tmpfs /mnt/hostfault:size=64m `
  rust:1.78-slim-bookworm@sha256:0fea967628dc796a2b9d1d57ddb3af3b3f0a35b6c8c0e23690dbe0ceb71a2dc9 bash -lc `
  "export DEBIAN_FRONTEND=noninteractive && `
   apt-get update -qq && apt-get install -y -qq --no-install-recommends build-essential && `
   export PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin && `
   cd /workspace && export CARGO_TARGET_DIR=/tmp/pong-target `
   PONG_HOST_FAULT_ROOT=/mnt/hostfault && `
   cargo test --locked --test host_resource_faults `
   fi_14_real_enospc_during_cas_write_preserves_committed_history `
   -- --ignored --nocapture"
```

Repeat the command with each of these test names:

```text
fi_14_real_enospc_during_metadata_write_preserves_committed_history
fi_14_real_enospc_during_journal_write_preserves_committed_history
```

The evidence line records `filesystem=tmpfs`, raw Linux `ENOSPC` (errno 28),
the bytes used to exhaust the bounded volume, and Pong's observed status.
Expected status is `RESOURCE_EXHAUSTED` (or `RECOVERY_REQUIRED` where a
platform-specific recovery boundary requires it). Existing committed data
must remain readable, a rejected CAS write must leave no staging object, and
the metadata/journal mutations must not appear after a cold reopen.

The three Linux tests may be launched concurrently against one scratch root;
the host-fault lock serializes them because all three deliberately exhaust the
same filesystem. A failed or interrupted run leaves no held OS lock; remove
any stale child directory before reusing a root if a test was terminated while
cleaning up.

## Ordinary gate

The normal commands compile these tests but skip the host schedules:

```text
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```
