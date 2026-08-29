# Pong M1 Final Candidate Readiness

**Prepared:** 2026-08-29  
**Owner Decision:** `ACCEPTED`  
**Owner:** `NJHTR`  
**Date:** `2026-08-29`  
**Readiness:** `NOT_READY (FINAL_CI_PENDING)`

This is a candidate-freeze preparation snapshot, not a release tag. The Owner
has accepted the scope and governance decisions, and the source candidate is
frozen. Final GitHub-hosted Linux/macOS CI remains pending.

## Technical Gate

```text
TECHNICAL_GATE = PASS (scope-bounded evidence)
```

This technical result is limited to the explicit M1 boundary below. It does
not create a release candidate or release tag.

| Area | Scoped result | Boundary |
| --- | --- | --- |
| Build / Test / Clippy | `PASS` | Existing stable/MSRV and retained native evidence |
| Windows x86_64 / NTFS | `PASS` | Supported local row |
| Linux x86_64 / ext4 | `PASS` | Supported local row; Debian 13 Linux-native evidence retained |
| macOS | `PASS` qualification | GitHub-hosted runner only; filesystem `unknown` |
| FI-03 | `DEFERRED TO M2+` | No external provider execution claim; synthetic groundwork only |
| FI-07 | `PASS` bounded | Linux dedicated `tmpfs` resource model only |
| FI-07 other resource models | `NOT_APPLICABLE` to this M1 claim | No generic Windows NTFS/ext4/unknown-macOS resource guarantee |
| FI-13 | `PASS` bounded | Windows NTFS ACL and Linux ext4 POSIX evidence |
| FI-13 macOS local row | `NOT_APPLICABLE` | macOS is qualification-only |
| FI-14 | `PASS` bounded | Linux dedicated `tmpfs` CAS/metadata/journal model only |
| FI-14 other resource models | `NOT_APPLICABLE` to this M1 claim | No extrapolation to other filesystems |
| Property corpus | `PASS` bounded | Canonical 10,000 cases/property; Windows/Linux retained; hosted macOS qualification |
| Performance measurement | `PASS` measurement | Informational baseline only; not capacity certification |
| Event / Projection evidence | `PASS` technical evidence | ADR-0016 contract accepted by Owner; source ADR unchanged |
| Artifact consistency | `PASS` | Existing retained bundle checks |

`DEFERRED`, `NOT_APPLICABLE`, and `WAIVER_REQUIRED` are scope dispositions;
they are not relabeled as test passes.

## Governance Gate

```text
GOVERNANCE_GATE = ACCEPTED
```

The governance decisions and textual Owner declaration are recorded. The only
remaining release control is to freeze a clean committed candidate and
reconcile the evidence package against that exact commit.

## Accepted Scope

The Release Owner accepts:

1. Windows x86_64 / NTFS local support.
2. Linux x86_64 / ext4 local support.
3. macOS GitHub-hosted qualification only, with filesystem `unknown`.
4. Canonical 10,000 cases/property plus cross-platform qualification.
5. FI-13 Windows NTFS and Linux ext4 permission rows.
6. Linux `tmpfs` as the only formally verified M1 resource model.

Formal Owner acceptance is `ACCEPTED`; source candidate freeze is `FROZEN` at
`58e9e875cd5a781a94f921ec215e231cffdfafe6`.

## Deferred Scope

- FI-03 external side-effect reconciliation to M2+.
- Capacity certification beyond the informational performance baseline.
- SDK, CLI expansion, adapters, remote replication, and other post-M1 work.

## Accepted Exception

- Old Reader: `ACCEPTED EXCEPTION / OUT OF SCOPE`. No independent v0.1 reader
  exists.
- ADR-0015: `ACCEPTED AS INFORMATIONAL BASELINE`; capacity certification is
  not required for M1.

FI-03 is a scope deferment, not a test pass. Unclaimed resource models are
outside the M1 contract and are not evidence for those models.

## Remaining Technical Work

```text
CODING = 0
TESTING = 0 new coverage runs for the approved scope
EVIDENCE = 1 final candidate packaging/checksum reconciliation task
```

The retained evidence must be repackaged and rechecked after the Owner
authorizes the final freeze. The frozen commit must then receive the existing
quality-gate verification; that is release verification, not a newly invented
coverage requirement. No production-code change is required by the current
decision.

## Candidate Identity

| Item | Current value | Status |
| --- | --- | --- |
| HEAD | `58e9e875cd5a781a94f921ec215e231cffdfafe6` | Frozen source candidate; final CI pending |
| Branch | `dev` | Current branch |
| Initial evidence binding | `b20fc903eb7395f5d3f2a48a7f0184bcc3b02713` | Traceability/checksum baseline commit |
| Native workflow | `33145714975` | Predecessor evidence baseline at `6cb62fb...`; not final candidate CI |
| Windows evidence | Windows x86_64 / NTFS local native | Retained; no GitHub job required |
| Linux evidence | VMware Debian 13 Linux-native VM / ext4 | Retained at predecessor evidence commit `6cb62fb...` |
| macOS evidence | GitHub-hosted runner / filesystem `unknown` | Retained qualification |
| Artifact hashes | Existing bundle consistency pass | Recheck after final freeze |
| ADR-0015 | `Proposed` | Owner accepted informational baseline; source ADR unchanged |
| ADR-0016 | `Proposed` | Owner accepted M1 contract; source ADR unchanged |
| Working tree | `CLEAN` after freeze commit | Final CI still pending |
| Release tag | none | Do not create in this phase |
| Sign-off | `ACCEPTED` | NJHTR textual owner declaration recorded |

## Final Gate

```text
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
READINESS = NOT_READY (FINAL_CI_PENDING)
```

The technical evidence and governance decision are accepted, and the source
candidate is frozen. The release cannot be called ready until the final
GitHub-hosted Linux/macOS workflow runs against `58e9e875...` and its evidence
is rebound and verified. Windows is not a final GitHub CI requirement.

## Final Release Sequence

1. Repository owner freezes a clean candidate containing the accepted decision
   records and reconciles the evidence references/checksums against that exact
   commit.
3. Rerun artifact consistency and the complete quality gates for the frozen
   commit.
4. Release Owner reviews the frozen candidate. The governance decision is
   already accepted in the current sign-off.
5. Only after the clean candidate is verified may the normal release process
   create a tag.
