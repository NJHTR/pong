# Environment Model

**Implementation status:** A minimal, internally persisted `described`
environment exists for the M2 local-workspace slice. It is not a reproducible
environment contract. See
[`M2_WORKSPACE_SNAPSHOT_GATE.md`](../development/M2_WORKSPACE_SNAPSHOT_GATE.md).

## Purpose

An Environment describes the execution conditions needed to interpret or reproduce a workspace operation. It is versioned and referenced by workspace, snapshot, checkpoint, and operation records.

## Fields

- host OS and architecture
- runtime and language versions
- package manager and lockfile digests
- container image or sandbox profile
- network mode and approved endpoints
- mounted volume descriptors
- workspace provider and capabilities
- selected environment variables as redacted key/value metadata
- tool and adapter versions

## Capture policy

Capture an allowlisted, normalized summary. Never persist API keys, access tokens, passwords, private keys, cookies, raw authorization headers, or secret-bearing command arguments in ordinary history. Sensitive values are replaced with stable redaction markers; the original may exist only in an external secret manager under a policy-controlled reference.

The current `EnvironmentFacts` schema (`0.1`) captures only OS, architecture,
platform family, and explicitly allowlisted environment variables. Key names
that indicate secrets or host identity/path data are excluded. Values are
redacted before canonicalization and obvious token-shaped values that remain are
omitted. The facts are canonicalized and fingerprinted in the
`environment/v1` domain before being stored as immutable project-scoped
metadata.

An `environment_id` may be recorded idempotently with identical facts. Reusing
that identity with different facts or a different project is an integrity
error. A workspace may bind only an environment that exists in the same
project.

## Reproducibility levels

`described` means metadata is recorded; `reconstructible` means dependencies and image references are available; `reproducible` means a provider can materialize an equivalent environment; `identical` is not promised for nondeterministic services or external data.

The current implementation supports only the `described` level. It does not
capture runtime/package versions, lockfile digests, container images, network
policy, mounts, provider capabilities, or tool/adapter versions yet.

## Drift

The runtime compares current environment fingerprints with the referenced revision. Drift is an event and may block replay or commit according to policy. Environment changes create a new revision rather than mutating prior evidence.

## Security boundary

Environment capture is observability, not sandboxing. Network, process, filesystem, and secret controls belong to provider and host policy. Pong records the declared controls and any observed violation.
