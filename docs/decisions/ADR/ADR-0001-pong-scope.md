# ADR-0001: Pong Scope and Boundary

- Status: Accepted
- Date: 2026-08-19

## Context

Pong is intended to serve many agent frameworks and human operators. Confusing infrastructure with orchestration would couple the core to prompts, model providers, scheduling, or roles.

## Problem

What responsibility does Pong own, and what must remain outside it?

## Options

1. Build an opinionated multi-agent framework.
2. Provide only a file snapshot utility.
3. Provide framework-agnostic execution versioning and coordination context.

## Decision

Choose option 3. Pong owns identity, workspace/environment context, operations, events, version DAG, execution DAG, artifacts, checkpoints, and policy evidence. It does not reason, plan, schedule, assign roles, select LLMs, or orchestrate business messages.

## Consequences

Adapters and explicit APIs are first-class. Core schemas need stable, generic nouns and clear capture confidence. Some framework-native behavior remains outside Pong.

## Rejected alternatives

An orchestration framework would duplicate existing ecosystems; a file-only utility cannot answer what an agent did or resume it safely.

