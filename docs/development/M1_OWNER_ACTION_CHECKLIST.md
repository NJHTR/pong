# M1 Owner Action Checklist

**Purpose:** the smallest set of actions that only the Release Owner must
perform. Codex must not fill these decisions, sign a waiver, accept an ADR, or
create a release tag.

**Current gate:** `M1_RELEASE_GATE = BLOCKED / NOT_PASSED`  
**Candidate evidence commit:** `6cb62fb455e92ab731a4bb5233856d10c1f1ce93`  
**Native workflow:** `33145714975`

## MUST DECIDE

- [ ] Name the exact M1 supported scope: Windows x86_64/NTFS, Linux x86_64/ext4,
      and whether the macOS GitHub-hosted runner is qualification-only with
      filesystem `unknown`.
- [ ] Decide FI-03: keep real external side-effect execution as an M1 MUST, or
      explicitly exclude it and record `FI-03 = DEFERRED TO M2+`.
- [ ] Decide FI-07 resource scope. Either fund safe native ext4/NTFS schedules,
      or explicitly bound M1 to the measured Linux `tmpfs` resource model and
      record: `Pong M1 does not promise all filesystem resource-exhaustion
      semantics.`
- [ ] Decide FI-14 resource scope using the same boundary. Do not infer ext4,
      NTFS, or APFS behavior from Linux `tmpfs` evidence.
- [ ] Decide macOS FI-13 as `NOT_APPLICABLE / QUALIFICATION_ONLY`, or make
      macOS a local supported filesystem and provide a real filesystem-specific
      permission test. Do not call the current runner APFS or a physical Mac.
- [ ] Choose the property policy: recommended is one canonical retained
      10,000-case corpus plus accepted-platform qualification; a per-platform
      10,000 policy is a separate, higher-cost choice.
- [ ] Choose the Old Reader disposition: provide a genuine independent v0.1
      reader, sign a compatibility exception, or define unreleased pre-M1
      formats as out of scope.
- [ ] Decide ADR-0015: informational performance baseline, or explicit hard
      performance gate with accepted thresholds.
- [ ] Decide ADR-0016: accept or reject the proposed projection contract.

## MUST SIGN

- [ ] Complete `docs/development/M1_RELEASE_OWNER_SIGNOFF.md` with:
      owner name, exact commit/scope, accepted evidence, exceptions, residual
      risks, decision, date, and signature.
- [ ] For every exception, state the affected capability/platform/filesystem,
      exact commit, risk, compensating control, owner, and date.

## MUST PROVIDE

- [ ] If Old Reader is not excluded, provide the real historical v0.1 binary or
      archival build, immutable SHA-256, reader-created fixture, read/open and
      mutation probes, raw output, exit codes, and cold-reopen result.
- [ ] If FI-03 remains an M1 MUST, provide or authorize the real external
      provider/tool harness and its effect-reconciliation evidence.
- [ ] If FI-07/FI-14 ext4 or NTFS rows remain in scope, provide the safe
      disposable filesystem/quota environment and retained native records.
- [ ] If Policy B is selected, provide the additional per-platform normative
      property corpus records instead of relying on standard-suite logs.

## OPTIONAL

- [ ] Authorize additional ext4, NTFS, or hosted-macOS diagnostic evidence
      beyond the scope required by the chosen M1 contract. Codex/platform
      owners perform the execution; the Release Owner does not run it.
- [ ] Choose the candidate version/tag name after all required rows and signed
      exceptions are complete. Codex does not create the tag in this phase.

## DEFERRED

- [ ] External provider execution, side-effect reconciliation, and related
      FI-03 evidence may move to M2+ only after the Owner excludes that claim
      from M1 in the signed scope.
- [ ] Future capacity qualification beyond an informational M1 performance
      baseline may move beyond M1 only after the ADR-0015 decision records that
      boundary.
- [ ] Node/Python SDKs, CLI expansion, framework adapters, remote replication,
      and other post-M1 roadmap work remain deferred. They are not substitutes
      for the decisions above.

## Exact Owner Order

1. Name the owner and freeze the supported platform/filesystem/resource scope.
2. Decide FI-03, FI-07, FI-14, macOS FI-13, and the property policy.
3. Decide the Old Reader disposition and provide the reader only if compatibility
   is retained.
4. Decide ADR-0015 and ADR-0016 without editing them automatically.
5. Record all exceptions, residual risks, compensating controls, date, and exact
   commit in the sign-off record.
6. Authorize candidate freeze. Codex/repository owners then regenerate
   references/checksums, rerun consistency and full gates, and report the clean
   candidate state; these are not manual Release Owner tasks.
7. Review that exact candidate and complete the final `ACCEPTED` or `REJECTED`
   sign-off. Only then authorize a formal release tag.

Until this list is completed, the project remains in decision preparation:

```text
M1_RELEASE_GATE = BLOCKED / NOT_PASSED
```
