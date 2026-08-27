# CLI Design

**Status: Proposed contract.** The CLI is a thin projection of the Agent Protocol. It must never invent behavior that the SDK or protocol does not define. Output is human-readable by default and stable JSON with `--json`; errors use protocol error codes and a non-zero exit status.

## Common rules

- All commands resolve the project from the current directory or `--project`; paths are logical workspace-relative paths.
- State-changing commands accept `--request-id` and relevant `--expected-head`/`--lease-epoch`; retries reuse the same request ID.
- `--dry-run` is mandatory for planning destructive rollback, replay, merge, and external-effect approval.
- Agents use `--json`, cursors, and explicit `--actor`/session credentials. Humans get concise summaries and actionable conflict details.
- Permission failures, stale refs, unknown side effects, and recovery-required states are never converted into success.

## Command contract

| Command | Purpose; input | Output | Permissions | Agent and human use | Failure cases |
| --- | --- | --- | --- | --- | --- |
| `pong init` | Create a project manifest and `.pong`; input path, policy, optional project name | Project ID, format/protocol versions, initial branch/workspace choice | Create project, write target directory | Agent bootstraps a project; human initializes a repository | Existing/incompatible `.pong`, unsafe path, permission denied, interrupted transaction |
| `pong status` | Read current agent/workspace/branch/head, dirty and degraded state; optional consistency/cursor | Machine-readable context, pending operations, capture confidence, warnings | Read workspace/project | Agent decides next action; human scans current state | No project/session, stale cache, recovery required |
| `pong agents` | List registry with filters/status/cursor | Agent IDs, sessions, workspace, branch, task, last operation/checkpoint | Inspect agents | Agent discovers peers; human monitors activity | Unauthorized peer visibility, unavailable projection |
| `pong agent inspect <agent>` | Inspect one identity, capabilities, leases, recent events | Redacted detail and provenance | Inspect requested agent | Same structured view for both | Unknown ID, sensitive fields denied |
| `pong workspace list` | List logical workspaces with driver/status/lease | Workspace summaries and cursor | Read project/workspaces | Agent selects an isolated workspace; human audits placement | Provider unavailable, stale registry |
| `pong workspace inspect <workspace>` | Inspect materialization, environment, branch/head, drift | Binding, capabilities, snapshot and reconciliation status | Read workspace/environment summary | Agent checks where it is; human diagnoses drift | Unknown workspace, lease/recovery conflict |
| `pong env list` | List safe environment revisions/fingerprints | Allowlisted facts, redaction markers, current binding | Read environment summary | Agent checks compatibility; human reviews drift | Secret policy violation, unavailable provider |
| `pong branch` | Show current branch or list refs; optional `--all` | Branch names, heads, protection/lease status | Read refs | Both use for navigation | No current workspace, inaccessible ref |
| `pong branch create <name>` | Create ref from current or `--from <commit>` | New branch ID/name/head event | Create branch | Agent creates task branch; human starts review branch | Name collision, missing base, policy denied |
| `pong checkout <branch>` | Materialize a branch into current workspace; optional `--detach`, `--force` | New head, changed paths, safety snapshot ID | Write workspace and checkout ref | Agent switches isolated context; human switches deliberately | Dirty/uncommitted changes, lease loss, conflict, provider failure |
| `pong log` | Traverse commit or execution history; filters/cursor | Commits/events with parents, actor, task, operation ranges | Read history | Agent gathers context; human reviews provenance | Invalid cursor, pruned evidence, unauthorized payload |
| `pong diff` | Compare workspace/commit/branch/checkpoint; optional resource scope | File/resource changes, operation evidence, conflict markers | Read compared resources | Both inspect before merge/commit | Missing object, unreconciled workspace, unsupported resource diff |
| `pong show <commit>` | Inspect immutable commit, snapshot manifest, provenance | Commit metadata and safe tree/artifact references | Read commit/artifacts | Agent evaluates peer work; human reviews | Unknown/integrity-invalid commit, redacted payload |
| `pong checkpoint` | Create or list checkpoints; options task, label, scope, `--request-id` | Checkpoint ID, snapshot/cursor, restore exclusions | Create/read checkpoint, workspace read | Agent marks resumable state; human names recovery point | Missing adapter state, dirty capture, quota, policy denial |
| `pong commit` | Create semantic commit from workspace; message, parents, artifacts, expected head | Commit ID, advanced ref, warnings and operation range | Read workspace, create commit, update ref | Agent commits intentional progress; human records a reviewable change | No lease, stale head, unresolved conflicts, secret scan, object failure |
| `pong rollback <target>` | Plan/apply selected local dimensions from commit/checkpoint; requires `--dry-run` then approval for apply | Plan, safety snapshot, applied restore event, excluded effects | Restore/approve workspace or agent state | Agent requests and verifies plan; human approves risky restore | Unknown side effects, dirty target, lease loss, integrity/recovery error |
| `pong replay <operation-or-range>` | Plan/apply eligible operations in an isolated workspace | Per-operation plan/result, new lineage, skipped/approval records | Replay capability; side-effect approval | Agent can replay deterministic work; human approves external effects | `SIDE_EFFECT_UNKNOWN`, unsupported capability, missing artifact, policy denied |
| `pong merge <branch>` | Plan/apply three-way version merge into current branch/workspace | Base/ours/theirs, conflict records, merge commit on success | Read refs, write workspace, create merge commit | Agent resolves declared conflicts; human reviews result | Stale head, unresolved conflict, incompatible schema/resource |
| `pong activity` | Stream/query recent events by agent/workspace/task; cursor and consistency | Redacted events, sequence, capture confidence, loss markers | Read audit/activity scope | Agent polls context; human tails diagnostics | Cursor expired, unauthorized stream, degraded/lost records |

## Exit statuses

`0` success; `1` command rejected or conflict; `2` invalid invocation; `3` authorization/security failure; `4` recovery or integrity required; `5` transport/storage unavailable; `6` operation outcome unknown. Scripts must use JSON `error.code`, not prose or only exit status.

