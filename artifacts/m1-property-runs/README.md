# M1 Property Run Records

These records retain explicit case counts and fixed seeds. They are evidence
inputs only; they do not accept the property gate or the M1 release gate.

## Windows stable continuation audit

`windows-stable-properties-10000-2026-08-26.json` records a run with
`PONG_PROPTEST_CASES=10000` in the independent
`target/windows-stable-audit-test` directory.

- PT-01/02/03/04/05/06/07/08/11/12: 10,000 cases each, all passed in
  382.38 seconds.
- PT-10/14: started with 10,000 cases but were interrupted before any result;
  the aggregate record intentionally claims no pass for them.
- PT-09 also has a separate no-result record at
  `windows-stable-pt09-10000-2026-08-26.json`. The test was still running at
  the 60-second audit boundary and was stopped with Ctrl+C (observed exit code
  `1`); its case count is intentionally `null` and no property result is
  inferred from the interruption. This is retained as historical audit
  evidence and is superseded by the completed rerun below.
- `windows-stable-pt09-10000-rerun-2026-08-26.json` records the completed
  normative PT-09 rerun: 10,000/10,000 cases passed in 1,081.76 seconds with
  exit code `0`. Its raw log SHA-256 is
  `62D59E034CA30AD0DB1EC3BD1FACE31359C3BC00B65F7F180FD7ADC3B5308E6F`.
- `windows-stable-pt10-10000-rerun-2026-08-26.json` records the completed
  normative PT-10 rerun: 10,000/10,000 cases passed in 1,325.25 seconds with
  exit code `0`. Its raw log is 2,496 bytes and SHA-256 is
  `7AFBC2921F1BED25C43CE8DA77EE40D0C734E61EA98E0F87AF1C3BEBA0562B3E`.
- PT-13 and FI-10 remain unimplemented and are not part of this run.
- Raw log SHA-256:
  `E90B809B2019D8EE0F7439D39E666D92354D75FE361599F74621961F67586CB6`

The interrupted storage-heavy aggregate run and its structured no-result
record remain retained to make the audit boundary visible. PT-10 and PT-14
now have completed Windows stable normative records; PT-14 still requires a
normative run on each other accepted platform/filesystem row. A passing
default `cargo test` run is not equivalent to this normative evidence.

## Windows stable PT-14 normative rerun

`windows-stable-pt14-10000-rerun-2026-08-27.json` records the isolated
Windows NTFS stable-Rust run. `PONG_PROPTEST_CASES=10000` was distributed over
9 migration failpoints as 1,112 cases per point, for 10,008 executed cases;
the test reported `1 passed; 0 failed` and exit code `0`. Test execution took
1,376.66 seconds (1,400.947 seconds wall-clock including startup). No host
error or mixed-generation state was observed. The raw stdout log is retained
at `windows-stable-pt14-10000-rerun-2026-08-27.log` with SHA-256
`5CA3901D442AA8C2F69EFC08AED925703526D4C5536CA5F34694B45AE8F90171`.
The raw log was reconstructed from captured command output without rerunning;
the JSON records that capture method. This is Windows stable evidence only,
not release-owner acceptance or cross-platform coverage.

## Concurrent PT-14 probe

Three concurrent focused PT-14 probes were run on Windows NTFS with
`PONG_PROPTEST_CASES=18` (the nine migration failpoints plus retained
regressions). All three exited zero; these are supplemental concurrency
records and do not close the normative 10,000-case requirement:

| Run | Raw log | SHA-256 |
| --- | --- | --- |
| 1 | `pt14-concurrent-probe-1.log` | `BA46A12050A8AAC0676018340EC9ADB5E02A5C858C56DE0C3A1C4B406366F1F9` |
| 2 | `pt14-concurrent-probe-2.log` | `BA58D962A0056BD17B5736539D6DBC0128E460F09C4477BA0E4B00EA5CB46A14` |
| 3 | `pt14-concurrent-probe-3.log` | `DC90DE7DCCEC9F9B5992EC23E4849DB9CD9F61A24CB8469C4A1C3A6E4D7BEFC8` |

On 2026-08-27, three additional bounded probes ran concurrently with
`PONG_PROPTEST_CASES=256` in independent target directories. Each covered all
nine migration failpoints (29 generated cases per point), exited zero, and
reported `1 passed; 0 failed` in 43.75-45.31 seconds. No host error was
observed. The structured record and raw-log hashes are retained in
[`windows-stable-pt14-diagnostic-2026-08-27.json`](windows-stable-pt14-diagnostic-2026-08-27.json).
The trace run in that record emits one JSON line per failpoint, retry
operation, and cold reopen when `PONG_PT14_TRACE=1`; it observed one attempt
per operation, no host errors, and no mixed-generation state. This is
supplemental diagnostic evidence only: it does not provide a normative
10,000-case PT-14 run or release-owner acceptance.
