# Pong Project Constitution

**Status: Normative.** This constitution governs design and implementation. A pull request or change that conflicts with it must include an ADR explaining the exception.

1. **Documentation First.** Product behavior and domain terms are documented before implementation.
2. **Architecture Before Implementation.** A milestone has a reviewed design and test strategy before production code begins.
3. **No undocumented architecture decisions.** Durable choices live in an ADR or an explicit update to an existing ADR.
4. **No silent API changes.** Contract changes are versioned, announced, and tested.
5. **No breaking changes without an ADR.** Compatibility impact and migration are mandatory sections.
6. **Every operation must be observable.** Best-effort capture is labeled with method and confidence; absence is not hidden.
7. **Every recoverable state must be recoverable.** A recovery claim names the exact boundary, prerequisites, and known loss.
8. **Every irreversible operation must be explicit.** Approval, idempotency, and audit evidence are required before execution where Pong controls the boundary.
9. **Core must remain framework-agnostic.** Framework behavior belongs behind adapters and cannot leak into Core schemas.
10. **Runtime must remain extensible.** Tool and driver capabilities use versioned extension points rather than closed enums.
11. **Workspace must be abstracted from the physical filesystem.** A path, container, host, and pod are driver details.
12. **Agent must be a first-class identity.** Provenance, permissions, registry state, and session lineage are not optional metadata.
13. **Concurrency must be explicit.** Lease, compare-and-swap, ordering, and conflict behavior are documented for each mutable aggregate.
14. **Security must be designed before implementation.** Trust boundaries, secret handling, and least privilege are acceptance criteria.
15. **Tests must be designed before core implementation.** Invariants, crash cases, compatibility fixtures, and security tests precede production code.

## Change discipline

Every change names its affected contracts, updates cross-references, and adds a migration or explicit non-goal. Generated or experimental material must be marked as such and cannot be imported by production packages. A review may reject a feature that increases capture or security claims without evidence.

