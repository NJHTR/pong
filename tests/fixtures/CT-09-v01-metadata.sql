PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;

CREATE TABLE repository_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
CREATE TABLE refs (
    name TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE idempotency (
    project_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    command_digest TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (project_id, actor_id, request_id)
);
CREATE TABLE events (
    event_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    stream_id TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    schema_version TEXT NOT NULL,
    actor_id TEXT,
    request_id TEXT,
    occurred_at TEXT NOT NULL,
    payload_digest TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    UNIQUE (stream_id, sequence)
);
CREATE TABLE operation_journal (
    project_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    operation_id TEXT NOT NULL,
    phase TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (project_id, actor_id, request_id)
);

INSERT INTO repository_meta(key, value)
    VALUES ('repository_format', '0.1');
INSERT INTO repository_meta(key, value)
    VALUES ('redaction_profile_id', 'default');
INSERT INTO repository_meta(key, value)
    VALUES ('redaction_profile_version', '0.1');
INSERT INTO refs(name, value, updated_at)
    VALUES ('refs/heads/main', 'legacy-head', 'v01-time');
INSERT INTO events(
    event_id, project_id, stream_id, sequence, schema_version,
    actor_id, request_id, occurred_at, payload_digest, payload_json
) VALUES (
    'legacy-event', 'legacy-project', 'legacy-stream', 1, '0.1',
    NULL, NULL, 'v01-time', 'sha256:legacy-payload',
    '{"type":"legacy.event","opaque":true}'
);
