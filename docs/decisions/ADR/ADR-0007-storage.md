# ADR-0007: Transactional Metadata plus Content-Addressed Objects

- Status: Accepted
- Date: 2026-08-19

## Context

Events and mutable projections need transactions and queries; file trees and tool artifacts need deduplication and streaming.

## Problem

Which storage architecture meets local-first durability without freezing future scale?

## Options

1. Put everything in a relational database.
2. Put everything in Git objects/files.
3. Use a local transactional metadata/event store and a separate CAS/object/artifact store behind ports.

## Decision

Choose option 3. v0.1 may use SQLite-compatible metadata/WAL and filesystem CAS. PostgreSQL, RocksDB, packs, and object storage are later drivers, not Core concepts.

## Consequences

Atomic metadata transitions and content deduplication are possible. Cross-store commit requires a manifest protocol and recovery reconciliation.

## Rejected alternatives

Single-store designs either make large payloads awkward or make queries/transactions fragile; hard-coding a vendor blocks local-first migration.

