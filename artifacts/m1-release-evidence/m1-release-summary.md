# M1 Release Evidence Summary

Updated 2026-08-29 from frozen source candidate
`58e9e875cd5a781a94f921ec215e231cffdfafe6`. This is an accepted-governance
evidence bundle with final-CI evidence for the frozen candidate. It is not a
tagged release artifact.

## Decision

`M1 Release Gate: READY FOR RELEASE (TAG NOT CREATED)`

The bounded M1 technical gate is `PASS`, and Release Owner `NJHTR` accepted
the governance decisions on 2026-08-29 by textual owner declaration. Source
candidate freeze is complete at `58e9e875...`; final-CI evidence binding is
recorded at `b20fc903...`. GitHub-hosted Run `33253638226` passed Linux and
macOS, with all 13 commands per platform returning zero. Windows is covered by
retained local native evidence and has no final GitHub job requirement. No
release tag exists.

## Evidence disposition

| Area | Status | Release status | Reason |
| --- | --- | --- | --- |
| Technical gate | PASS | PASS | Scope-bounded build, test, clippy, platform, fault, property, projection, performance-measurement, final-CI, and evidence checks pass. |
| Windows x86_64 / NTFS | PASS | PASS | Required M1 platform; retained Windows local native evidence is accepted; no GitHub job is required. |
| Linux x86_64 / ext4 | PASS | PASS | Required M1 platform; VMware Debian 13 Linux-native support evidence plus final GitHub-hosted CI Run `33253638226`. |
| macOS qualification | PASS | PASS_QUALIFICATION | macOS GitHub-hosted runner only; filesystem is `unknown`, with no physical-Mac or APFS claim; final CI Run `33253638226`. |
| FI-03 | DEFERRED TO M2+ | BLOCKED | Owner-approved scope deferment; this is not a test pass. |
| FI-07 / FI-14 | PASS (bounded) | BLOCKED | Only the retained Linux `tmpfs` resource model is claimed. |
| FI-13 | PASS (bounded) | BLOCKED | Windows NTFS and Linux ext4 permission evidence; macOS permission fault is not required. |
| Property policy | PASS (bounded) | BLOCKED | Canonical 10,000 cases/property plus accepted-platform qualification. |
| Old Reader | ACCEPTED EXCEPTION | BLOCKED | No backward-compatibility guarantee for unreleased/internal pre-M1 formats. |
| Performance | PASS measurement | BLOCKED | Accepted as informational baseline, not capacity certification; source ADR-0015 remains `Proposed`. |
| Projection contract | ACCEPTED | BLOCKED | Owner accepted ADR-0016 as the M1 contract; source ADR remains `Proposed`. |
| Release owner | ACCEPTED | BLOCKED | NJHTR, 2026-08-29, textual owner declaration. |
| Candidate freeze | FROZEN | READY_FOR_RELEASE | Source commit `58e9e875...` is cleanly committed; final CI Run `33253638226` is reconciled. Release tag remains owner-controlled and uncreated. |

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
