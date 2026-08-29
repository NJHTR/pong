# Pong M1 Old Reader Compatibility Exception

**Status:** `ACCEPTED EXCEPTION / OUT OF SCOPE`  
**Owner:** `NJHTR`  
**Date:** `2026-08-29`  
**Formal exception:** `ACCEPTED`

## M1 First Formal Release Baseline

M1 is Pong's first formal release baseline. The repository history and
retained artifacts contain no independently released Pong v0.1 reader, tag,
binary, immutable binary hash, or reader-created fixture. A current-source
reader or fixture is not an old reader.

## Proposed Scope Exception

The Release Owner accepted the following compatibility boundary:

```text
No backwards compatibility guarantee for unreleased/internal pre-M1 repository formats.
```

This excludes unreleased or internal pre-M1 repository states from the M1
backward-read guarantee. It does not claim that an old reader exists or that a
legacy repository was successfully opened.

## Risk

Users holding unreleased or internal pre-M1 repositories may be unable to open
or migrate them with M1. Current-reader migration and selector/generation
integrity tests do not remove that compatibility risk.

## Scope and Controls

- Supported M1 compatibility claim: current M1 repository format and the
  migration/integrity behavior evidenced in the retained bundle.
- Excluded claim: backward compatibility with an independently released v0.1
  reader or unreleased/internal pre-M1 formats.
- Control: publish the exclusion explicitly in the signed Owner record and do
  not describe current-source fixtures as old-reader evidence.
- The evidence baseline commit, owner, date, residual risk, and textual
  declaration are recorded in `M1_RELEASE_OWNER_SIGNOFF.md`; the final clean
  candidate commit remains to be frozen.

## Future Compatibility Policy

Any future compatibility guarantee requires an independently versioned reader
or fixture, immutable provenance, read/open and mutation probes, raw output,
exit codes, and cold-reopen evidence. No fake reader may be introduced to
retroactively close this M1 gap.

## Owner Acceptance

| Field | Value |
| --- | --- |
| Status | `ACCEPTED EXCEPTION / OUT OF SCOPE` |
| Reason | No independently released v0.1 reader exists. |
| Scope | No backward compatibility guarantee for unreleased/internal pre-M1 formats. |
| Risk | Existing historical/internal repositories may require migration or may be unreadable. |

Owner identity and acceptance were explicitly supplied by the project owner in
this execution. The exception is a textual owner decision, not a digital
certificate. It is now effective for the M1 scope:

```text
OLD_READER = ACCEPTED EXCEPTION / OUT OF SCOPE
FORMAL_EXCEPTION = ACCEPTED
```
