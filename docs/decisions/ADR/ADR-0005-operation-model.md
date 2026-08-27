# ADR-0005: Extensible Operation Envelope

- Status: Accepted
- Date: 2026-08-19

## Context

Tool ecosystems evolve faster than a fixed operation enum. Pong still needs searchable, typed records and clear reversibility semantics.

## Problem

How should heterogeneous calls be represented without an unmaintainable list?

## Options

1. Closed enum of tools.
2. Unstructured text logs.
3. Versioned capability-qualified `tool`/`action` envelope with typed references and policy classifications.

## Decision

Choose option 3. Core validates the envelope and lifecycle; adapters register names and schemas. Input/output are redacted or content-addressed references. Reversibility, replayability, side effect, and capture confidence are explicit fields.

## Consequences

New tools do not require Core releases, but schema governance and redaction tests are required. Queries must understand extensions gracefully.

## Rejected alternatives

Closed enums block integration; text logs cannot support reliable diff, replay, or policy.

