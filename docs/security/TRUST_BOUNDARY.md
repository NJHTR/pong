# Trust Boundary

## Zones

1. **Core metadata**: object manifests, refs, event index, and policy; trusted to enforce invariants.
2. **Runtime supervisor**: journals operations and mediates tools; semi-trusted because hooks can be bypassed.
3. **Agent process/framework**: untrusted input and potentially compromised behavior.
4. **Workspace environment**: files, processes, containers, network, and databases; trust depends on deployment.
5. **External providers**: APIs and services outside Pong control.

Crossing a zone requires an authenticated actor, capability decision, normalized resource, and durable audit event. Data crossing out is redacted according to the destination policy.

## Boundary assumptions

Local-first v0.x assumes the host filesystem protects `.pong` from ordinary workspace writes and that the operator controls the machine. It does not assume agents are mutually trustworthy. A containerized deployment may move the runtime boundary outward, but the manifest must state whether the host, container runtime, and network are trusted.

## Confused deputy prevention

The runtime never uses its own broad privileges on behalf of an agent without carrying the agent's session and capability set. Framework adapters cannot mint identities or approve side effects. Paths and resources are resolved within the authorized workspace before execution.

## Data boundary

Raw tool payloads, environment values, and external responses may contain sensitive data. The default boundary exports summaries and digests, not raw bodies. Explicit export requires policy, redaction, and audit. Importing an artifact marks its provenance and trust level; untrusted content is not executable by default.

