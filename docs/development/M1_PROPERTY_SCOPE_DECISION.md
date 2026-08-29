# M1 Property Scope Decision

**Prepared:** 2026-08-28  
**Audited commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Decision authority:** Release Owner  
**Current status:** `NOT_PROVEN` (no owner scope acceptance recorded)

This document records the interpretation of the existing M1 contract. It does
not add a Gate condition, promote evidence, or accept a release scope.

## Execution Policy Proposal (Pending Owner Acceptance)

For this M1 release-baseline decision, the proposed policy is:

- **Primary normative corpus:** 10,000 cases per property, deterministic and
  retained. This is one normative CI corpus, not an automatic 10,000-case
  multiplication for every platform.
- **Cross-platform qualification:** native smoke and selected corpus evidence
  on each required platform row. Existing Windows and Linux records exceed the
  selected-corpus requirement; the GitHub-hosted macOS row has quality-gate
  plus selected property evidence.
- **Required platform scope:** Windows x86_64; Linux x86_64 on ext4; and a
  GitHub-hosted macOS runner. The hosted macOS filesystem remains `unknown` and
  is not described as APFS or a physical Mac.
- **Acceptance authority:** the Release Owner must freeze this policy and the
  exact accepted rows in `M1_RELEASE_OWNER_SIGNOFF.md`.

This proposal narrows the interpretation of the evidence package; it does not
rewrite the existing `MUST PASS` property invariants or promote the current
`NOT_PROVEN` status before owner acceptance.

The phrase "smoke and selected corpus" is a qualification policy proposal, not
a claim that unlisted PT rows passed on every platform. Each PT-01 through
PT-14 row that remains in the accepted M1 matrix still needs the Gate-required
evidence, or an explicit bounded Owner exception naming the excluded row and
its residual risk. No property status is promoted by this document.

## Contract Reading

The existing [`M1_DURABLE_PRIMITIVES_GATE.md`](M1_DURABLE_PRIMITIVES_GATE.md)
defines PT-01 through PT-14 as `MUST PASS` properties. It specifies a
deterministic generated corpus with a default minimum of **10,000 cases per
property in CI**, retaining seeds and shrink traces on failure. The same gate
also says that contract, property, and fault rows must be green on every
supported local filesystem/platform.

The contract does **not** state that the 10,000-case corpus must be run
independently on every platform. The closure plan and compatibility matrix do
require evidence for every row that the owner accepts as supported. Those are
platform-qualification and acceptance requirements; they must not be silently
converted into a new per-platform 10,000-case rule.

Therefore the defensible interpretation is:

1. The normative corpus is 10,000 cases per property for the declared CI
   property run, with PT-13/PT-14 also carrying cold-reopen and identity
   assertions.
2. Every platform/filesystem that the owner accepts for M1 needs retained
   platform evidence and the property result needed to justify that row.
3. A stricter policy of 10,000 cases on every accepted row is allowed only as
   an explicit owner decision; it is not implied by the current wording.

PT-13 is the fixed event/projection fixture and migration-contract suite; it
is not a generated 10,000-case property. Its acceptance remains coupled to
ADR-0016. PT-14 is the generated migration property whose retained normative
record reports 10,008 executions because the harness spans nine failpoints.

## Platform Qualification

No platform/filesystem row is formally accepted. The compatibility matrix
explicitly says **Current release scope: none accepted**.

| Platform / filesystem | Existing evidence | Corpus record | Qualification |
| --- | --- | --- | --- |
| Windows x86_64 / NTFS | Stable and MSRV quality gates; retained property records | PT-01..PT-12: 10,000 each; PT-09/PT-10 reruns: 10,000 each; PT-14: 10,008 across 9 failpoints | Normative evidence for this row; owner acceptance pending |
| Linux VM `rtc-node-1` / ext4 | Rust 1.85 Linux-native quality gates | PT-01..PT-08, PT-09, PT-10, PT-11, PT-12: 10,000; PT-14: 10,008 | Linux-native evidence; not MSRV evidence; owner acceptance pending |
| GitHub-hosted Linux runner / ext4 | Run `33145714975`, all 13 command exits `0` | Standard property suites executed; no separate retained per-property normative record | Hosted executable evidence; owner acceptance pending |
| GitHub-hosted macOS runner / filesystem `unknown` | Run `33145714975`, all 13 command exits `0` | No complete retained per-property normative corpus record | Do not call this APFS or a physical Mac; owner must decide whether this row is supported |
| Docker overlay, Docker ext4 volume, Linux `tmpfs` | Existing executable and fault evidence | Supplemental records | Do not widen support to these filesystems without owner acceptance |

The Linux VM records are real Linux-native evidence for the declared VM and
filesystem. They do not establish Rust 1.78 MSRV because the VM ran Rust 1.85,
and they do not by themselves accept any release row.

## Evidence Requirement

For each PT row that is used in the release decision, retain:

- property ID and invariant identity;
- generator/corpus version, seed, requested and executed case count;
- raw output and exit code;
- platform, filesystem, repository commit, and toolchain identity;
- shrink/failure trace when a case fails; and
- cold-reopen/source-event or generation-identity checks for PT-13/PT-14.

The retained Windows and Linux VM records satisfy this shape for the rows they
cover. The macOS GitHub-hosted artifact is useful platform evidence, but it is
not a retained 10,000-case per-property corpus.

## Scope Decision

| Question | Finding |
| --- | --- |
| Accepted platforms | None formally accepted yet |
| Normative corpus definition | 10,000 cases per property in CI, deterministic and retained |
| Required case count | 10,000 executed cases per property for the normative CI corpus; PT-14 records 10,008 because its harness spans 9 migration failpoints |
| Explicit per-platform 10,000 rule | Not present in the current contract |
| Platform qualification | Required for every row the owner accepts |
| Current property status | `NOT_PROVEN` |
| PT-13 dependency | Release acceptance remains coupled to ADR-0016 |
| Owner decision required | Freeze accepted rows and state whether the normative corpus is one CI corpus or per accepted row |

## Recommended Disposition

Do not create a new “macOS 10,000-case blocker” solely because the contract
does not say “per platform.” Under the proposed policy, the macOS row needs
platform qualification and selected property evidence, not an inferred
10,000-case-per-property claim. Keep the overall property row `NOT_PROVEN`
until the owner freezes the accepted platform/filesystem scope and accepts the
one-CI-corpus policy. If the owner instead chooses a per-row normative policy,
the missing macOS and any other accepted-row records become technical
follow-up work.

This is a scope choice, not a test-standard reduction: no existing failure is
hidden and no 10,000-case result is inferred where none was retained.
