# Security Model

The current internal M2 implementation enforces only the local metadata,
locator, path, redaction, and snapshot-object checks described in its acceptance
gate. It is not a sandbox and does not yet enforce process/network/container
isolation or capture all workspace mutations.

## Security objective

Pong protects agent workspaces, execution history, artifacts, and credentials while remaining useful for audit and recovery. It is an integrity and authorization boundary for recorded state; it is not automatically a complete sandbox. Sandboxing is delegated to the runtime/environment and must be declared.

## Assets

Assets include source and generated files, operation inputs/outputs, agent identity, task context, branch refs, snapshots/commits, environment metadata, artifacts, audit logs, and secret material. Availability and provenance matter alongside confidentiality: a forged event can cause unsafe replay even if no secret leaks.

## Threats

Threats include malicious or compromised agents, prompt-driven exfiltration, path traversal, command injection, confused-deputy adapters, forged runtime events, replay of stale commands, secret leakage in logs, malicious artifacts, concurrent writer races, and local disk theft. The model assumes the host OS may be stronger or weaker than Pong; assumptions are documented per deployment.

## Controls

- Capability-based authorization on every command and resource.
- Workspace and agent isolation through OS/container policy where required.
- Canonical path and argument validation; no shell interpolation by Pong.
- Secret detection/redaction before journaling or persistence.
- Append-only audit events with hash chaining and optional signatures.
- Content-addressed blobs with size/type limits and malware scanning hooks.
- Approval gates for irreversible or high-impact operations.
- Encryption at rest and in transit when storage leaves the trusted host.
- Rate limits, quotas, and bounded journals for denial-of-service resistance.

## Secret handling

Secrets are referenced by opaque handles and injected at execution time. Environment snapshots use an allowlist; values matching configured secret providers, key formats, or high-entropy detectors are replaced with redaction markers. A redacted value is never recoverable from normal commit, diff, replay, or debug output. Access to a secret-bearing operation requires a separate capability and is itself audited.

## Audit and provenance

Audit records capture actor, capability, decision, resource, request ID, and result. Provenance links an artifact to producing operation, runtime, and input digests. Hashing detects accidental or unauthorized modification; signatures should be enabled when evidence crosses trust boundaries.

## Incident response

On suspected compromise, revoke tokens, freeze writes, preserve the event journal, quarantine affected artifacts, rotate credentials, and produce a timeline from audit events. Recovery may restore workspace state but cannot erase evidence or reverse external side effects.

## Limitations

Pong cannot guarantee that an untrusted host did not read local files, that an external API honored a request, or that an opaque tool was fully observed. Deployments requiring those guarantees must place the runtime behind a stronger sandbox and use provider-side audit logs.
