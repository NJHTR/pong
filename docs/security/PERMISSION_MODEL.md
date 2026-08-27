# Permission Model

## Capability vocabulary

Capabilities are scoped tuples: `verb:resource[:selector]`. Core verbs include `read`, `inspect`, `write`, `execute`, `create`, `merge`, `rollback`, `replay`, `approve`, `admin`, and `read_secrets`. Resources include project, workspace, branch, operation, artifact, environment, task, message, and audit stream.

Examples: `read:workspace:ws_123`, `write:workspace:ws_123`, `create:commit:branch/main`, `replay:operation:op_9`, `approve:side_effect:network`, `read_secrets:none` (explicit deny).

## Evaluation

Authorization evaluates actor identity, token capabilities, project policy, workspace ownership, branch protection, operation sensitivity, and current lease/revision. Deny is the default. A broader project grant cannot bypass an explicit resource deny. Adapter-supplied claims are untrusted until mapped to a registered agent session.

## Roles and defaults

The human owner may administer a project. An agent receives read access to its attached project/workspace, write access to its own workspace, and commit/checkpoint rights subject to branch policy. Shared workspaces grant read-only access unless a lease is acquired. CI or automation uses narrowly scoped service identities.

## Approval gates

Irreversible actions (external writes, production deploys, payments, destructive database commands) require `approve:side_effect` and a policy-defined human or delegated approver. Approval binds to an operation digest, target, expiry, and actor; changing any of these invalidates approval.

## Leases and revocation

Write leases carry an epoch and expiry. Every mutation presents the epoch; stale leases fail. Tokens and leases can be revoked; revocation is checked before execution and before commit publication. In-flight operations become `unknown` and require reconciliation.

## Policy distribution

Policies live in versioned project configuration with a local immutable copy for offline operation. Changes are events, require authorization, and take effect at a defined sequence boundary. A policy hash is included in operation and commit metadata for auditability.

