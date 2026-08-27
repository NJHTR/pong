# Operation

**Status: Normative.** An Operation is one observed attempt by an agent/session to invoke a tool or mutate an execution resource. It is the atomic unit of observability, not necessarily of versioning.

The current Rust Core implementation is the bounded internal ledger described
in [`M3_OPERATION_LEDGER_GATE.md`](../development/M3_OPERATION_LEDGER_GATE.md):
it records envelopes and lifecycle facts below Runtime, SDK, CLI, server, and
framework adapters. Recording an operation is not itself evidence that an
external effect was observed or reversible.

The extensible envelope includes `operation_id`, `schema_version`, agent/session/workspace/environment IDs, optional `parent_operation_id`, logical and wall-clock timestamps, `tool`, `action`, typed input/output references, resource, before/after state attestations, result/error, reversibility class, replayability class, side-effect classification, policy decision, and redaction metadata. Large payloads live in the object/artifact store and are referenced by digest.

Operations use capability-qualified names such as `filesystem.write` or `process.exec`; adapters may add names without changing Core. The lifecycle is `started -> completed|failed|cancelled|unknown`. Recording quality is a separate state: `durable` or `unreconciled` when execution evidence exists but publication is incomplete. A capture record is durable before a caller receives success when the driver can enforce that ordering; otherwise the record is marked best-effort.

Classes:

| Class | Meaning | Example |
| --- | --- | --- |
| `REVERSIBLE` | Pong can produce a compensating change from captured before-state | write, delete, rename |
| `REPLAYABLE` | The intent/input can be attempted again, but result may differ | search, image generation |
| `IRREVERSIBLE` | External effect cannot be safely compensated by Pong | payment, email, deploy |

These labels are independent: an operation may be reversible and replayable, or replayable but not reversible. Policy, approval, and evidence are attached to the operation rather than inferred from a tool name.

## Illustrative operation families

The following names are compatibility vocabulary, not a closed enum. Adapters may add a capability-qualified name while preserving the same envelope:

| Family | Illustrative actions |
| --- | --- |
| Filesystem | `READ_FILE`, `WRITE_FILE`, `CREATE_FILE`, `DELETE_FILE`, `MOVE_FILE`, `COPY_FILE` |
| Process | `SHELL_EXEC`, `PROCESS_START`, `PROCESS_STOP` |
| Web/browser | `WEB_SEARCH`, `BROWSER_OPEN`, `BROWSER_CLICK`, `BROWSER_INPUT` |
| Generation | `IMAGE_GENERATE` |
| Database | `DB_QUERY`, `DB_INSERT`, `DB_UPDATE`, `DB_DELETE` |
| Agent lifecycle/collaboration | `AGENT_MESSAGE`, `AGENT_SPAWN`, `AGENT_TERMINATE` |

The normalized `tool` and `action` fields carry the adapter's canonical spelling; the family is useful for policy defaults but never replaces resource-specific authorization.
