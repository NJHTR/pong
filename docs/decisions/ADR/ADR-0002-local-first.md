# ADR-0002: Local-First v0.x

- Status: Accepted
- Date: 2026-08-19

## Context

Agents often run offline, in sandboxes, or on developer machines. A distributed control plane would add operational and consistency cost before the object model is validated.

## Problem

Where should the first durable implementation live?

## Options

1. Distributed server first.
2. Local-first metadata/WAL and content store with future replication ports.
3. Ephemeral in-memory recorder.

## Decision

Choose option 2. v0.x is a single-project local repository with transactional metadata/events and a filesystem CAS behind interfaces. Replication is a later protocol, not a hidden dependency.

## Consequences

Offline use, inspectability, and deterministic crash tests are strong. Multi-host coordination and central registry are deferred; IDs and event envelopes must already support future sync.

## Rejected alternatives

Server-first overfits deployment and in-memory storage cannot meet recovery or audit requirements.

