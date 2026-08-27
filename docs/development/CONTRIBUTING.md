# Contributing

## Before opening a change

Read `docs/roadmap/NEXT_TASK.md`, relevant architecture/protocol documents, and the project constitution. Confirm whether the change is documentation-only, experimental, or production implementation. For a public contract or architectural choice, add an ADR before coding.

## Pull request expectations

Describe the problem, scope, invariants affected, security/privacy impact, failure behavior, migration/compatibility plan, and tests. Include protocol or schema examples for contract changes. Keep unrelated formatting and metadata churn out of the change.

## API and history discipline

Do not rewrite committed event history or change semantics behind a renamed field. Use additive fields, compensating events, migrations, and deprecation warnings. Every operation exposed to agents or humans must have authorization, idempotency, observability, and failure semantics documented.

## Documentation standards

Use concise Markdown, stable headings, ASCII by default, and normative terms consistently: MUST, SHOULD, MAY. Link to canonical concepts rather than duplicating definitions. Examples must be redacted and marked illustrative when not executable.

## Review and conduct

Reviews focus first on correctness, data loss, security, compatibility, and operability. Be specific and kind. A maintainer may request an ADR, test, or threat-model update before approval. Breaking changes require explicit release notes and migration guidance.

## Experimental work

PoCs live under `poc/`, state `EXPERIMENTAL / NOT PRODUCTION CODE`, and document assumptions, results, and retirement criteria. A PoC cannot become an implicit dependency of core.

