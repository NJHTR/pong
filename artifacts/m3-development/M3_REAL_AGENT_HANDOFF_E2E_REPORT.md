# M3 Real Agent Handoff E2E Validation

**Date:** 2026-09-11  
**Mode:** \`LOCAL_NATIVE_DEVELOPMENT\`  
**Platform:** Windows 11 / x86_64 / NTFS  
**Status:** \`PASS\` for the executed local integration

## Result Matrix

| Area | Result | Evidence |
| --- | --- | --- |
| Core Pong E2E | \`PASS\` | \`cargo test --locked --test agent_handoff_e2e -- --ignored --nocapture\` |
| Codex E2E | \`PASS\` | Real \`codex exec\`, exit code 0; duration recorded in the matching log |
| Claude Code E2E | \`PASS\` | Real \`claude -p\`, exit code 0; duration recorded in the matching log |
| Agent handoff | \`PASS\` | Durable \`handoff-codex-to-claude-real\` |
| Provenance | \`PASS\` | Task, executions, versions, checkpoints, handoff and resume IDs persisted |
| Workspace isolation | \`PASS\` | Separate locators and heads for W1/W2; source remains unchanged |
| Checkpoint durability | \`PASS\` | Repository cold reopen finds C1 and C2 |
| Recovery / continuation | \`PASS\` | Claude reads materialized Codex state and adds \`subtract\` |
| Provider-neutral control facade | \`PASS\` | \`cargo test --locked --test control_layer -- --nocapture\` |
| MCP / SDK integration | \`NOT_PROVEN\` | Intentionally outside this slice |

## Executed Workflow

1. Pong registered Codex and Claude Code identities, created Task
   \`task-real-agent-handoff\`, Workspace \`workspace-codex-real\`, and
   Execution \`execution-codex-real\`.
2. A real \`codex exec\` process created \`src/calculator.rs\` and
   \`README.md\` in W1, including \`multiply\` and the \`Added by Codex\` marker.
3. Pong captured a Snapshot, published a durable Codex Version (the exact
   generated ID is recorded in the structured JSON evidence), and created
   Checkpoint \`checkpoint-codex-real\`.
4. Pong interrupted E1, created W2 and resumed E2 from C1. The durable handoff
   relation is \`handoff-codex-to-claude-real\`; no lease or Version ownership
   was transferred from W1.
5. Pong materialized the source Version into W2, producing a W2-local Snapshot.
   A real \`claude -p\` process verified \`multiply\`, then added \`subtract\`
   and the \`Added by Claude Code\` marker.
6. Pong published a durable W2-local Version (the exact generated ID is
   recorded in the structured JSON evidence) and Checkpoint
   \`checkpoint-claude-real\`.
7. A cold repository reopen confirmed both checkpoints, the handoff, and the
   resume record.

## Invariants Verified

- Version ownership remained workspace-local; both resulting Versions have no
  foreign parent.
- The source Version, source Snapshot, W1 head, and W1 filesystem were unchanged
  by the handoff.
- W2 received target-local materialized state and its own Snapshot/head.
- Cross-workspace continuation used durable Pong IDs, not Git commits or hidden
  provider session state.
- Provider names are metadata only; no provider-specific core branch was added.

## Provider Evidence

The authoritative command output is in
\`m3-real-agent-handoff-e2e-windows-native-2026-09-11.log\`; structured metadata
is in the matching \`.json\` file. Codex emitted the created file contents and
Claude reported preservation of Codex markers plus the new subtract function.
Both processes ran with ephemeral/no-session-persistence settings in temporary
workspaces.

## Quality Gates

The focused real-agent test passed:

\`\`\`text
test real_codex_to_claude_handoff_through_pong ... ok
test result: ok; 1 passed; 0 failed; 0 ignored
\`\`\`

Repository-wide \`fmt\`, \`check\`, \`test\`, \`clippy\`, and
\`git diff --check\` all passed in the current working tree run. The exact
full-regression command was \`cargo test --all --locked --no-fail-fast\`:
\`538 passed; 0 failed; 276 ignored\` in 67.78 seconds. The 276 ignored
contract placeholders are reported separately and are not counted as passes.
The provider-neutral facade test passed with 1 passed, 0 failed, and 0
ignored. The real-agent test was rerun on 2026-09-11 after the facade and
documentation changes and passed with 1 passed, 0 failed, and 0 ignored.

## Architecture Gaps

\`BLOCKED\` or \`NOT_PROVEN\`: provider authentication policy, provider
adapters, MCP transport, network communication, and production deployment.
These are adapter/transport work and are not silently claimed as implemented by
this core validation.

## Files

- \`tests/agent_handoff_e2e.rs\`
- \`src/control.rs\`
- \`src/lib.rs\`
- \`tests/control_layer.rs\`
- \`docs/architecture/M4_PROVIDER_NEUTRAL_CONTROL_LAYER.md\`
- \`docs/decisions/ADR-M4-provider-neutral-control-layer.md\`
- \`artifacts/m3-development/m3-real-agent-handoff-e2e-windows-native-2026-09-11.json\`
- \`artifacts/m3-development/m3-real-agent-handoff-e2e-windows-native-2026-09-11.log\`
- \`docs/roadmap/NEXT_TASK.md\`
- \`docs/roadmap/CHANGELOG.md\`

No SQLite schema, release tag, or remote push was changed by this validation.
The local provider-neutral facade is production Rust source, but it is an
internal/test-gated composition layer and does not claim provider integration.
