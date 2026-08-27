# M1 Compatibility Evidence

The separately released historical Pong v0.1 reader is not present in this
workspace. The current Rust reader can open the retained v0.1-shaped fixture
and the selector contract can reject v0.2 before mutation, but those checks are
not evidence from the historical executable.

Required before acceptance:

- the v0.1 executable and immutable SHA-256;
- an isolated v0.2 selector/generation fixture;
- read, open, and mutation-before/after-selector probes;
- raw output, exit codes, and a cold-reopen inspection.

This directory intentionally contains no substitute binary. The normalized
release matrix records the row as `BLOCKED`.
