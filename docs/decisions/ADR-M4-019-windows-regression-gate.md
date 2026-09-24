# ADR-M4-019: Windows Regression Gate

**Date:** 2026-09-24
**Status:** Accepted for the Windows regression gate; Model A remains
`NOT_SUPPORTED`.

## Context

Windows regression evidence had mixed two different access models:

1. the supported topology, where one Core owns a Repository and multiple
   Runtime clients use the protocol; and
2. unsupported direct Repository access, where independent handles or
   processes open the same Repository.

The latter can observe Windows file-system errors during startup integrity
scans, including raw codes 2 and 33. Historical full-suite runs also recorded
`STATUS_ACCESS_VIOLATION` and `STATUS_HEAP_CORRUPTION`; their causal relation
to these fixtures was not proven.

## Decision

Keep the product architecture unchanged: one Core owner remains the supported
live path, and direct multi-process Repository access remains
`NOT_SUPPORTED`.

Test fixtures must:

- use an isolated `TempDir` Repository, SQLite database, Workspace and result
  files;
- wait for every child process before cold reopen or temporary-directory
  cleanup;
- separate Repository startup from the in-process concurrent publication phase;
- identify direct-access failure stages (`OPEN`, `WRITE`, `SNAPSHOT`);
- keep a deterministic Core-owner rejection test in the ordinary regression
  gate; and
- keep direct-access race tests available as an explicit
  `direct-access-stress` characterization suite.

Raw Windows codes are not globally accepted. Code 2, 5, 32 and 33 are only
interpreted with evidence from the operation phase; an unknown phase remains
`NOT_PROVEN`.

## Consequences

The default Windows suite can gate supported Core-owned behavior without
serializing Cargo tests, lowering concurrency, deleting tests, or widening
expected errors. The stress suite remains useful evidence about unsupported
direct access, but a passing run does not promote that topology to supported
product behavior.

The current full regression and explicit stress run pass on Windows. Historical
native crashes and the exact syscall behind a transient code 2 remain
documented residual risks, not silently reclassified successes. Linux/macOS
parity, TLS deployment and public Internet deployment remain outside this
gate.
