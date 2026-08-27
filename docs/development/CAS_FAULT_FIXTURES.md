# CAS Fault Fixtures

**Status: M1 test support; not production behavior.**

The CAS exposes an instance-scoped, one-shot `CasFailpoints` plan for
deterministic tests. The default constructors use a disabled plan and never
consult environment variables or process-global switches.

## Stable synthetic failures

`CasFailPoint` identifies the boundary under test:

- `StagingWrite` runs before the staging bytes are synced.
- `Publication` runs immediately before the no-replace hard link.
- `DirectorySync` runs after publication has made the object name visible.
- `StartupScan` runs before the existing CAS tree is scanned.

`CasFaultAction` provides these decisions:

- `ShortWrite(n)` writes a prefix and returns `FAULT_INJECTED:...:short_write`.
- `QuotaExhausted(n)` writes a prefix and returns the stable
  `RESOURCE_EXHAUSTED` code with a `quota_exhausted` diagnostic label.
- `PermissionDenied` returns the stable `PERMISSION_DENIED` code without
  changing host ACLs.
- `Fail` returns a boundary-only `FAULT_INJECTED` result.

All plans are consumed on their first matching boundary. A failed staging
operation removes the partial file. A directory-sync fault is intentionally
post-publication: the immutable object may be visible even though durability
of the containing directory is unconfirmed, and a retry verifies that object
instead of overwriting it.

## Platform evidence boundary

Synthetic permission and quota actions are required for deterministic Windows
coverage. They validate cleanup, error classification, no-partial-read, and
retry invariants without depending on local administrator rights, ACL policy,
file sharing state, filesystem type, or available free space.

They are **not** evidence that the host kernel, NTFS/ReFS, antivirus filters,
or a mounted network filesystem handles a real ACL revocation or `ENOSPC`
correctly. The M1 gate therefore also requires a separate platform matrix that
records real permission and quota runs where the environment can provide them.
If a platform cannot reproduce either condition reliably, the run must be
marked unsupported with the reason and the synthetic fixture result retained;
the implementation must not silently claim real-failure coverage.
