# Open Questions

These are intentionally unresolved. An implementation may not turn an answer into a silent convention; close each question with an ADR or an explicit deferral.

| ID | Question | Why it matters | Owner / exit evidence |
| --- | --- | --- | --- |
| OQ-001 | Can v0.1 guarantee pre-ack capture for every supported tool host? | Defines safety claims and reconciliation behavior | Runtime PoC with crash injection |
| OQ-002 | Which environment facts are stable enough to hash across hosts? | Affects cache, replay, and diff usefulness | Fingerprint corpus and redaction tests |
| OQ-003 | How should non-file resources participate in a merge? | File three-way merge does not cover DB/browser/remote state | Resource-specific merge policy prototype |
| OQ-004 | What is the minimum native state adapter for each initial framework? | Prevents lowest-common-denominator SDK | Two independent adapter experiments |
| OQ-005 | How much event history can local storage retain by default? | Balances auditability and disk/privacy costs | Retention benchmark and policy review |
| OQ-006 | Can operation payloads be encrypted per project without harming inspectability? | Sensitive traces need stronger isolation | Key-management design and failure test |
| OQ-007 | What semantics should `rollback` expose for a partially observed tool? | Prevents false claims of undo | Recovery state machine and user study |
| OQ-008 | When should remote replication become a supported contract? | Avoids premature distributed consistency promises | Local load test plus at least one sync PoC |
| OQ-009 | Is a separate Task aggregate needed in Core v0.1 or only an adapter projection? | Limits model surface while preserving execution context | Usage scenarios and ADR |
| OQ-010 | Which Git interoperability level is worth maintaining? | Full Git compatibility could distort the model | Import/export prototype and user feedback |

