# M1 Performance Raw Runs

These files are measurement evidence only. They do not accept ADR-0015 or
widen Pong's supported platform claim.

- Probe record: `m1-perf-0.3` with raw samples and p50/p95/p99/max summaries,
  including eight independent migration runs per probe.
- Workload seed: `5562315273442557953`.
- Run date: 2026-08-26.
- `Cargo.lock` SHA-256:
  `108E191A65F726519CF76791A080058887B4CF1EDBBF9D428BBF5553774DC2DE`.
- Stable toolchain: Rust 1.95.0, target directory
  `target/windows-stable-final`.
- MSRV toolchain: Rust 1.78.0, target directory
  `target/windows-msrv-178-final`.
- Linux overlay toolchain: Rust 1.78.0 in the pinned
  `rust:1.78-slim-bookworm` image, named target volume
  `pong-msrv178-slim-target` mounted at `/root/pong-target`.

Each run was executed from `D:\pong` with an isolated output path:

```powershell
$env:CARGO_TARGET_DIR = 'D:\pong\target\windows-stable-final'
$env:PONG_PERF_OUTPUT = 'D:\pong\artifacts\m1-performance-runs\windows-stable-run-1.json'
cargo test --locked --test performance_measurements -- --ignored --nocapture
```

The same command was repeated for runs 2 and 3 and with
`cargo +1.78.0` plus `target/windows-msrv-178-final` for the MSRV rows.
The Linux overlay rows used the equivalent command in the pinned container;
they are not native ext4 evidence.

| File | SHA-256 |
| --- | --- |
| `linux-overlay-run-1.json` | `7B6EA317361C737E7FEF836ACA9F25DA387049B0CFF8B0DDA52C3B4CF7588516` |
| `linux-overlay-run-2.json` | `979CFCDC0831639F473155A07E83F114744D6760421E31FB209D0FBF43B5CC2C` |
| `linux-overlay-run-3.json` | `B1020CB09926059AD914CC010F8AAE1BA596D57D814CB00E1C8E237DBF0557ED` |
| `windows-stable-run-1.json` | `851B3429DE8AD946C23CFFA9E67922B6D98253DCBA306EE113CDA65520513DD6` |
| `windows-stable-run-2.json` | `0189A912B5313B69E37CD08F052F96247474BD0B6B2F12163A68AB418E5929A3` |
| `windows-stable-run-3.json` | `669CF82FEF4F8175F5FFC8C18F70F76212124F9D79EFAE06AB98E928CA9F98CD` |
| `windows-msrv-178-run-1.json` | `A1EA8F56709499182E65A7C813081B5622A82BD1176549008DDDD2EA0C3A8620` |
| `windows-msrv-178-run-2.json` | `B637E74F12EC4F1DA6BD1815B9ED4785752D892B5556BC49B29C75A67AE1545E` |
| `windows-msrv-178-run-3.json` | `8B49077290EC78B4289343059703D8C49D2251D87415177F19DF733967017F4F` |

The current package still lacks three-run evidence for native Linux ext4,
Windows native disk-full, macOS, or any other unaccepted filesystem row.
