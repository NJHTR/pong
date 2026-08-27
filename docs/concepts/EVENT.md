# Event

**Status: Normative.** An Event is an append-only fact about a domain transition, such as agent registration, lease acquisition, operation completion, commit creation, branch movement, checkpoint restore, or policy denial.

An event has an event ID, schema version, project scope, aggregate/type and ID, causation and correlation IDs, logical sequence, producer session, timestamp, payload/reference digests, and integrity metadata. Events describe what happened; an Operation describes a tool attempt. One operation can emit several events, and an event can describe a non-tool transition.

The event log is ordered per project stream and causally linked across streams. Consumers must tolerate duplicate delivery and use event IDs/sequence to apply idempotently. Events are audit records, not a message queue or a guaranteed real-time collaboration channel.

