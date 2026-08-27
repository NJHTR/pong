# M1 Platform Run Artifacts

The retained ext4-backed Linux runs are executable evidence, not a release
support decision:

- `linux-ext4-rust-178-run-1-2026-08-26.json` records the pinned Rust 1.78
  image, Docker named volumes, commands, observed statuses, and log hashes. It
  is an initial run whose test temporary directory was not pinned to the
  volume.
- `linux-ext4-rust-178-run-2-2026-08-26.json` is the authoritative run: it
  sets `TMPDIR=/repo/tmp` on the ext4 volume so test repositories and their
  WAL/CAS/staging files use the same filesystem.
- `linux-ext4-rust-178-run-1-2026-08-26.log` and
  `linux-ext4-rust-178-run-2-2026-08-26.log` are the raw quality-gate outputs.
- `linux-ext4-filesystem-identity-2026-08-26.log` retains `df -T` output showing
  the repository volume mounted as `ext4`.
- `windows-stable-release-audit-2026-08-26.log` retains the final Windows
  stable quality-gate output (SHA-256:
  `C8A7E01116B5DFE7711810217920F78BEB9294B4C3CBD1B3E621CCBD67905E1E`).
- `windows-msrv-178-release-audit-final-2026-08-26.log` retains the final
  Windows Rust 1.78 quality-gate output after installing the matching
  `rustfmt` and `clippy` components (SHA-256:
  `1D02DFE1F940EABF564E73D912A84FBB7C7220B83C2587E8E68694D448C64E10`).

An earlier MSRV attempt in `windows-msrv-178-release-audit-2026-08-26.log`
is retained as an audit trail: it exposed the missing local `rustfmt`
component before the final rerun. It is not the authoritative pass record.

The repository was copied into `pong-linux-ext4-audit-repo`; build output was
kept in the independent `pong-linux-ext4-audit-build` volume. The authoritative
run passed
`cargo fmt --all -- --check`, `cargo check --locked`, `cargo test --locked`,
and `cargo clippy --locked --all-targets -- -D warnings`.

This Docker-VM filesystem row must not be relabeled as native Linux ext4,
macOS, Windows disk-full, or a separately released historical reader. The
M1 release owner still must accept the platform matrix and all other release
artifacts.
