# M1 Release Evidence Summary

Updated 2026-08-29 from frozen source candidate
`58e9e875cd5a781a94f921ec215e231cffdfafe6`. This is an accepted-governance
evidence bundle, not a final-CI release artifact.

## Decision

`M1 Release Gate: BLOCKED / NOT PASSED`

The bounded M1 technical gate is `PASS`, and Release Owner `NJHTR` accepted
the governance decisions on 2026-08-29 by textual owner declaration. Source
candidate freeze is complete at `58e9e875...`; final CI has not yet run against
that commit, so release readiness remains blocked. No release tag exists.

## Evidence disposition

| Area | Status | Release status | Reason |
| --- | --- | --- | --- |
| Technical gate | PASS | BLOCKED | Scope-bounded build, test, clippy, platform, fault, property, projection, and evidence checks pass. Final CI is pending for the frozen source candidate. |
| Windows x86_64 / NTFS | PASS | BLOCKED | Required M1 platform; retained native evidence is accepted. |
| Linux x86_64 / ext4 | PASS | BLOCKED | Required M1 platform; Debian 13 Linux-native evidence is retained. |
| macOS qualification | PASS | BLOCKED | macOS GitHub-hosted runner only; filesystem is `unknown`, with no physical-Mac or APFS claim. |
| FI-03 | DEFERRED TO M2+ | BLOCKED | Owner-approved scope deferment; this is not a test pass. |
| FI-07 / FI-14 | PASS (bounded) | BLOCKED | Only the retained Linux `tmpfs` resource model is claimed. |
| FI-13 | PASS (bounded) | BLOCKED | Windows NTFS and Linux ext4 permission evidence; macOS permission fault is not required. |
| Property policy | PASS (bounded) | BLOCKED | Canonical 10,000 cases/property plus accepted-platform qualification. |
| Old Reader | ACCEPTED EXCEPTION | BLOCKED | No backward-compatibility guarantee for unreleased/internal pre-M1 formats. |
| Performance | PASS measurement | BLOCKED | Accepted as informational baseline, not capacity certification; source ADR-0015 remains `Proposed`. |
| Projection contract | ACCEPTED | BLOCKED | Owner accepted ADR-0016 as the M1 contract; source ADR remains `Proposed`. |
| Release owner | ACCEPTED | BLOCKED | NJHTR, 2026-08-29, textual owner declaration. |
| Candidate freeze | FROZEN | BLOCKED | Source commit `58e9e875...` is cleanly committed; final CI and tag remain pending. |

## Artifact use

`m1-release-matrix.json` is the machine-readable normalized index. Its entry
rows preserve evidence-time dispositions; its `decision_overlay` records the
later Owner acceptance. The
`build/`, `platform/`, `compatibility/`, `fault/`, `performance/`, and `logs/`
directories retain copied evidence or explicit absence notes. `SHA256SUMS`
covers every file in this bundle except itself. Source paths are preserved in
the artifact references manifest. The three `windows-stable-close-run-*`
performance files and logs are fresh measurement-only reruns from this
close-out; they do not accept ADR-0015.

Do not use this bundle to claim a tagged release, physical-Mac/APFS evidence,
capacity certification, public API, CLI, SDK, server, or M2 readiness.
