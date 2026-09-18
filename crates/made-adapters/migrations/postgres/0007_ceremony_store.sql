-- Shared ceremony persistence for multi-replica service deployments.
-- Domain values remain serialized as versioned JSON bytes; scalar columns
-- provide transaction boundaries, ordering and keyset pagination.

CREATE TABLE ceremony_store_global_position (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    last_position BIGINT NOT NULL CHECK (last_position >= 0)
);

INSERT INTO ceremony_store_global_position (singleton, last_position)
VALUES (TRUE, 0)
ON CONFLICT (singleton) DO NOTHING;

CREATE TABLE ceremony_streams (
    stream_id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 0)
);

CREATE TABLE ceremony_events (
    stream_id TEXT NOT NULL REFERENCES ceremony_streams(stream_id) ON DELETE CASCADE,
    sequence BIGINT NOT NULL CHECK (sequence > 0),
    global_position BIGINT NOT NULL UNIQUE CHECK (global_position > 0),
    event_id TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (stream_id, sequence),
    UNIQUE (stream_id, event_id)
);

CREATE INDEX ceremony_events_global_position_idx
    ON ceremony_events(global_position);

CREATE TABLE ceremony_snapshots (
    stream_id TEXT NOT NULL,
    version BIGINT NOT NULL CHECK (version >= 0),
    payload BYTEA NOT NULL,
    PRIMARY KEY (stream_id, version)
);

CREATE TABLE ceremony_publications (
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    digest TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (name, version)
);

CREATE TABLE ceremony_event_cursors (
    consumer TEXT PRIMARY KEY,
    payload BYTEA
);

CREATE TABLE ceremony_event_quarantine (
    consumer TEXT NOT NULL,
    global_position BIGINT NOT NULL CHECK (global_position > 0),
    payload BYTEA NOT NULL,
    PRIMARY KEY (consumer, global_position)
);

CREATE TABLE ceremony_memory_writes (
    write_id BIGSERIAL PRIMARY KEY,
    scope TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload BYTEA NOT NULL,
    UNIQUE (scope, idempotency_key)
);

CREATE INDEX ceremony_memory_writes_scope_idx
    ON ceremony_memory_writes(scope, write_id);

CREATE TABLE ceremony_execution_operations (
    operation_id TEXT PRIMARY KEY,
    payload BYTEA NOT NULL
);

CREATE TABLE ceremony_execution_intents (
    operation_id TEXT NOT NULL REFERENCES ceremony_execution_operations(operation_id),
    claim_fence TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (operation_id, claim_fence)
);

CREATE TABLE ceremony_execution_receipts (
    operation_id TEXT PRIMARY KEY REFERENCES ceremony_execution_operations(operation_id),
    receipt_id TEXT NOT NULL UNIQUE,
    observed_at TIMESTAMPTZ NOT NULL,
    payload BYTEA NOT NULL
);
