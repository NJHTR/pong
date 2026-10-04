# M4-021 Final Acceptance

**Date:** 2026-10-05
**Slice:** M4-021 provider-neutral external runtime interoperability
**Result:** `PASS / COMPLETE`
**Source:** `a05169e` (`test: add m4-021 operator mode`)
**Platform:** Windows native
**Provider launch:** `USER CONTROLLED`

Codex and Claude Code were started by the user as independent sessions. The
Pong operator remained provider-agnostic and handled only Protocol, durable
state, handoff, and verification.

## Acceptance Matrix

| Gate | Result |
| --- | --- |
| Codex provider | PASS |
| Claude provider | PASS |
| W1 | PASS |
| C1 | PASS |
| Handoff | PASS |
| W2 | PASS |
| C2 | PASS |
| E2 completion | PASS |
| Fresh-process inspection | PASS |
| Repository cold reopen | PASS |
| `Workspace.head` | PASS; equals `snapshot.root_digest` |
| Snapshot identity | PASS; equals `snapshot_id` |

## Durable Evidence

| Record | Value |
| --- | --- |
| Task | `task-m4-real-protocol-handoff` |
| W1 | `workspace-m4-real-codex` |
| W2 | `workspace-m4-real-claude` |
| E1 | `execution-m4-real-codex` (`interrupted`) |
| E2 | `execution-m4-real-claude` (`completed`) |
| C1 | `checkpoint-m4-real-codex` |
| C2 | `checkpoint-m4-real-claude` |
| Handoff | `handoff-m4-real-codex-claude` (`completed`) |
| Source Version | `ver-937994af53be68cbf8499f674bccce369a06446230459342f88f6ecc0a0db001` |
| Target Version | `ver-06d5f3c176eac0e9183a6b6e7ca275d45c98237537f172a8a671600a3e1d960c` |
| Source Snapshot | `snp-a3438fc783d87d108a8fee00d49b402a6934f81f25eddbc5f28ebed94ba61b4a` |
| Target Snapshot | `snp-164f3994f80cc78cd5d26915f2717f31137202535e94440b336514fde2e58eff` |
| W1 head / root digest | `sha256:a3438fc783d87d108a8fee00d49b402a6934f81f25eddbc5f28ebed94ba61b4a` |
| W2 head / root digest | `sha256:164f3994f80cc78cd5d26915f2717f31137202535e94440b336514fde2e58eff` |

The handoff manifest is retained at `D:\pong-m421-final\m4-021-handoff.json`.
The final temporary Repository is not copied into this repository. The
post-run Protocol inspection and `Repository::open` verification confirmed the
durable records above without provider credentials or workspace contents being
persisted here.

## Boundary Confirmation

No production Core, Protocol v1.0, provider configuration, or test contract
was changed for this closure. Runtime Integration remains outside the route;
E1/E2 remain experimental/reconsidered compatibility surfaces and E3 remains
stopped.
