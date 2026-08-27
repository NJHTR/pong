# M1 Fault Run Records

These records retain raw output from real host-resource schedules. They are
evidence inputs, not release-owner acceptance and do not change the M1 gate.

## Windows FI-13 ACL revocation

- Test: `tests/host_resource_faults.rs::fi_13_real_windows_acl_read_revocation_fails_closed_and_recovers`
- Platform/filesystem: Windows x86_64 MSVC on NTFS
- Toolchain: Rust 1.95.0
- Result: exit code `0`; kernel `PermissionDenied` / raw error `5`; Pong
  `PERMISSION_DENIED`; ACL restoration, cold reopen, and committed ref/CAS
  reads passed.
- Structured record: `windows-fi13-acl-audit-2026-08-26.json`
- Raw stdout: `windows-fi13-acl-audit-2026-08-26.log`
- Raw stdout SHA-256:
  `111F94C5DDC4AE99A24D1507E25C4177266C03CA130C4E9660FD661B933F1A36`

This is one real NTFS run. FI-13 remains a partial platform matrix until the
other accepted filesystem rows and the external cold-restart schedule exist.
Windows native disk-full evidence is deliberately not attempted on the
development volume.

## Linux FI-14 ENOSPC

Three independent runs used the pinned `rust:1.78-slim-bookworm` image and a
fresh 64 MiB Docker `tmpfs` for the scratch root. Each run reached real Linux
`ENOSPC` (`errno=28`), returned Pong `RESOURCE_EXHAUSTED`, preserved committed
history, and passed its cold-reopen assertions. The records remain partial
platform evidence; they do not cover Windows native disk-full/quota or every
declared filesystem.

- CAS write: `linux-fi14-cas-2026-08-26.json` and `linux-fi14-cas-2026-08-26.log`
- Metadata write: `linux-fi14-metadata-2026-08-26.json` and `linux-fi14-metadata-2026-08-26.log`
- Journal write: `linux-fi14-journal-2026-08-26.json` and `linux-fi14-journal-2026-08-26.log`
