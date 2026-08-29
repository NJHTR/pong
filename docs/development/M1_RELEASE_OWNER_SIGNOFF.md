# M1 Release Owner Sign-off

This is a human approval record. The Release Owner explicitly supplied the
identity, scope decisions, acceptance, date, and textual signature recorded
below. Codex did not invent an identity, image signature, digital certificate,
or detached signature artifact.

The supplied scope decision is recorded separately as:

```text
OWNER_DECISION = ACCEPTED
  FINAL_CANDIDATE_FREEZE = FROZEN (source commit 58e9e875cd5a781a94f921ec215e231cffdfafe6)
```

This acceptance closes the named governance decisions and authorizes final
candidate freeze preparation. It does not freeze a candidate or authorize a
release tag.

| Field | Value |
|---|---|
| Release | Pong M1 |
| Candidate Commit | `58e9e875cd5a781a94f921ec215e231cffdfafe6` (frozen source candidate); Run #5 evidence baseline predecessor `6cb62fb...` |
| Workflow | `33145714975` |
| Supported Scope | Windows x86_64/NTFS and Linux x86_64/ext4 required; macOS GitHub-hosted qualification only, filesystem `unknown` |
| Known Limitations | No external provider execution/reconciliation claim; Linux `tmpfs` is the only qualified resource-exhaustion model; performance is informational, not capacity-certified; no physical-Mac or APFS claim |
| Exceptions | Pre-M1 unreleased/internal repository formats are outside backward-compatibility scope |
| ADR-0015 | `ACCEPTED AS INFORMATIONAL BASELINE`; capacity certification not required for M1 |
| ADR-0016 | `ACCEPTED` as the M1 Projection Contract |
| Old Reader | `ACCEPTED EXCEPTION / OUT OF SCOPE` |
| FI-03 | `DEFERRED TO M2+` |
| Fault Scope | FI-07/FI-14 bounded to verified Linux `tmpfs`; FI-13 Windows NTFS and Linux ext4 pass; macOS permission fault not required |
| Property Policy | Canonical 10,000 cases/property plus accepted-platform qualification; no per-platform multiplication requirement |
| Decision | `ACCEPTED` |
| Owner | NJHTR |
| Date | 2026-08-29 |
| Signature | NJHTR |
| Signature Type | Textual owner declaration |
| Candidate Freeze | `FROZEN` (source commit `58e9e875cd5a781a94f921ec215e231cffdfafe6`) |

## Technical Review Inputs (Not Approval)

The current evidence snapshot identifies frozen source candidate commit
`58e9e875cd5a781a94f921ec215e231cffdfafe6`; native workflow `33145714975`
remains predecessor evidence bound to `6cb62fb...`. The Owner has accepted the
scope and decisions. Final CI for the frozen source candidate is still pending.

The current working tree was clean immediately after the freeze commit and no
release tag exists. Codex must not describe the candidate as release-ready
until final CI is run against `58e9e875...`.

## Required Review Inputs

The owner must review:

- [`M1_FINAL_GATE_AUDIT.md`](M1_FINAL_GATE_AUDIT.md)
- [`M1_FINAL_GATE_CLOSURE_PLAN.md`](M1_FINAL_GATE_CLOSURE_PLAN.md)
- [`M1_EVIDENCE.md`](M1_EVIDENCE.md)
- [`M1_COMPATIBILITY_MATRIX.md`](M1_COMPATIBILITY_MATRIX.md)
- [`M1_PERFORMANCE_ACCEPTANCE.md`](M1_PERFORMANCE_ACCEPTANCE.md)
- `artifacts/m1-release-evidence/` and its `SHA256SUMS`

Owner identity and acceptance were explicitly supplied by the project owner in
this execution. The signature above is a textual owner declaration. It is not
an independent digital signature certificate.
