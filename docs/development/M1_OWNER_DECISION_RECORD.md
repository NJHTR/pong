# Pong M1 Owner Decision Record

**Status:** `OWNER_DECISION = ACCEPTED` - the Release Owner explicitly supplied
the identity, date, scope decisions, exception acceptance, ADR decisions, and
textual signature recorded here. This is not a candidate freeze or release
tag.

| Formal control | State |
| --- | --- |
| Owner Decision | `ACCEPTED` |
| Formal Owner acceptance | `ACCEPTED` |
| Owner | NJHTR |
| Date | 2026-08-29 |
| Signature | NJHTR |
| Signature Type | Textual owner declaration |
| Candidate Freeze | `PENDING` |

```text
OWNER_SIGNATURE = NJHTR
OWNER_DATE = 2026-08-29
SIGNATURE_TYPE = Textual owner declaration
```

## Release Scope

The following is the formally accepted M1 release boundary.

- M1 is a durable local-core release baseline for Agent execution state.
- Required local support rows: Windows x86_64/NTFS and Linux
  x86_64/ext4.
- macOS is GitHub-hosted CI qualification only. The runner
  filesystem is `unknown`; this is not a physical-Mac or APFS claim.
- Docker overlay, Docker-backed ext4 volumes, and the dedicated Linux `tmpfs`
  mount are evidence environments, not additional support claims.

## Supported Platforms

| Platform | Proposed M1 role | Evidence boundary | Owner decision |
| --- | --- | --- | --- |
| Windows x86_64 | Local supported platform | NTFS evidence and Windows stable/MSRV gates | `ACCEPTED` |
| Linux x86_64 | Local supported platform | ext4 evidence from the Debian 13 Linux-native VM | `ACCEPTED` |
| macOS | GitHub-hosted qualification | Hosted runner evidence; filesystem `unknown` | `ACCEPTED AS QUALIFICATION` |

The macOS row must not be described as a physical Mac or as APFS evidence.

## Supported Filesystems

| Platform | Proposed filesystem scope | Resource-fault boundary | Owner decision |
| --- | --- | --- | --- |
| Windows x86_64 | NTFS | Permission evidence is retained; generic quota/disk-full behavior is not inferred | `ACCEPTED` |
| Linux x86_64 | ext4 | Permission evidence is retained; `tmpfs` resource evidence does not generalize to ext4 | `ACCEPTED` |
| macOS qualification | `unknown` | No filesystem-specific resource or permission claim | `ACCEPTED AS QUALIFICATION` |

## Fault Scope

The following dispositions are approved for final-candidate preparation in the
Owner Decision Draft. Formal scope acceptance remains pending:

- **FI-03:** Recommend `DEFERRED TO M2+`. The current Core has no independent
  external provider/tool execution hook. Existing synthetic unresolved-intent
  schedules must not be described as real external-side-effect evidence.
- **FI-07:** Retain the Linux dedicated `tmpfs` resource qualification. Do not
  extrapolate it to generic Linux ext4 or Windows NTFS quota/disk-full
  semantics without separately retained evidence.
- **FI-13:** Windows NTFS and Linux ext4 permission rows have retained evidence.
  macOS remains qualification-only with no local filesystem claim.
- **FI-14:** Retain Linux `tmpfs` CAS/metadata/journal resource qualification.
  Do not extrapolate it to other filesystems.
- Other FI-01..FI-14 rows remain subject to the existing fault matrix and its
  declared evidence classes. No synthetic row is promoted to native or
  external evidence here.

Effective fault scope: `ACCEPTED`.

## Property Policy

Approved-for-finalization draft policy:

- One canonical deterministic normative corpus of **10,000 cases per
  generated property** (PT-14 may retain 10,008 cases because its harness
  covers nine migration failpoints).
- PT-13 remains a fixed event/projection contract suite, not a generated
  10,000-case property.
- Windows and Linux retained 10,000-case records are evidence for their
  declared rows.
- macOS GitHub-hosted evidence remains platform qualification; no macOS
  10,000-case corpus is inferred.
- The policy does not multiply 10,000 cases by every platform unless the Owner
  explicitly chooses that separate policy.

Property policy: `ACCEPTED`.

## Old Reader Policy

The repository history and retained artifacts contain no independent,
formally released Pong v0.1 reader. A current-source reader or fixture is not
an old reader.

Approved-for-finalization first-formal-release boundary:

```text
No backwards compatibility guarantee for unreleased/internal pre-M1 repository formats.
```

This is a compatibility-scope exception proposal, not a test pass. The Owner
must either accept this bounded exclusion, or provide a genuine historical
reader, immutable hash, reader-created fixture, and executable probes.

Old Reader disposition: `ACCEPTED EXCEPTION / OUT OF SCOPE`.

## Performance Policy

The retained `m1-perf-0.3` package is measurement evidence for recorded
workloads. The proposed M1 policy is an **informational performance baseline**
with explicit platform/filesystem and workload limits, not an unlimited
capacity certification.

ADR-0015 remains `Proposed` and is not modified or accepted by this record.
The Owner must either accept ADR-0015 for a named scope after its required
measurements are present, or approve a bounded baseline/capacity exception
with excluded rows, operating limits, and residual risk.

ADR-0015 decision: `ACCEPTED AS INFORMATIONAL BASELINE`.

## Projection Contract

Technical evidence for the current recorded scope exists in the projection
implementation, `tests/event_projection.rs`, `tests/pt13_fi10.rs`, retained
PT-13/FI-10 artifacts, and the focused GitHub-hosted Run `33145714975`.
Evidence covers event envelopes, generation binding, cursor/ledger identity,
rebuild, idempotency, unknown-event handling, and FI-10 failpoint schedules.

ADR-0016 remains `Proposed`. The Core/Release Owner must review the retained
fixtures and explicitly accept or reject the projection contract; this record
does not change the ADR.

ADR-0016 decision: `ACCEPTED` as the M1 Projection Contract.

## Deferred to M2+

These are accepted roadmap boundaries:

- Real external provider/tool execution and FI-03 side-effect reconciliation.
- Broader filesystem-specific quota/resource guarantees beyond the measured
  Linux `tmpfs` qualification model.
- Capacity qualification beyond the bounded informational M1 performance
  baseline.
- SDKs, CLI expansion, framework adapters, remote replication, and other
  post-M1 roadmap work.

## Accepted Exceptions

**Formal accepted exception:** Old Reader is `ACCEPTED EXCEPTION / OUT OF
SCOPE` for this first formal release baseline, subject to the exact boundary
and risk in `M1_OLD_READER_EXCEPTION.md`.
Any exception must name the exact capability/platform/filesystem, candidate
commit, residual risk, compensating control, owner, and date.

## Known Risks

- Without a historical reader or an explicit compatibility exclusion, legacy
  pre-M1 repositories may not be readable.
- `tmpfs` resource evidence does not establish ext4, NTFS, or hosted-macOS
  resource semantics.
- No M1 claim is made for reconciling a real external provider effect after
  interruption while FI-03 is deferred.
- Hosted macOS filesystem identity is unknown and cannot support APFS or
  physical-host claims.
- The final committed candidate freeze and post-freeze verification are still
  outstanding.

## Release Decision

| Field | Value |
| --- | --- |
| Release Decision | `ACCEPTED` |
| Owner | NJHTR |
| Date | 2026-08-29 |
| Candidate freeze | `PENDING` |
| Signature | NJHTR (textual owner declaration) |

The governance decision is accepted. Until a clean committed candidate is
frozen and verified, the authoritative release state remains:

```text
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```
