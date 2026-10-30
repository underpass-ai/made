-- Shared protocol persistence for multi-replica service deployments.
-- Domain values remain serialized as versioned JSON bytes; scalar columns
-- provide transaction boundaries, ordering and keyset pagination.

CREATE TABLE protocol_store_global_position (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    last_position BIGINT NOT NULL CHECK (last_position >= 0)
);

INSERT INTO protocol_store_global_position (singleton, last_position)
VALUES (TRUE, 0)
ON CONFLICT (singleton) DO NOTHING;

CREATE TABLE protocol_streams (
    stream_id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 0)
);

CREATE TABLE protocol_events (
    stream_id TEXT NOT NULL REFERENCES protocol_streams(stream_id) ON DELETE CASCADE,
    sequence BIGINT NOT NULL CHECK (sequence > 0),
    global_position BIGINT NOT NULL UNIQUE CHECK (global_position > 0),
    event_id TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (stream_id, sequence),
    UNIQUE (stream_id, event_id)
);

CREATE INDEX protocol_events_global_position_idx
    ON protocol_events(global_position);

CREATE TABLE protocol_snapshots (
    stream_id TEXT NOT NULL,
    version BIGINT NOT NULL CHECK (version >= 0),
    payload BYTEA NOT NULL,
    PRIMARY KEY (stream_id, version)
);

CREATE TABLE protocol_publications (
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    digest TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (name, version)
);

CREATE TABLE protocol_event_cursors (
    consumer TEXT PRIMARY KEY,
    payload BYTEA
);

CREATE TABLE protocol_event_quarantine (
    consumer TEXT NOT NULL,
    global_position BIGINT NOT NULL CHECK (global_position > 0),
    payload BYTEA NOT NULL,
    PRIMARY KEY (consumer, global_position)
);

CREATE TABLE protocol_memory_writes (
    write_id BIGSERIAL PRIMARY KEY,
    scope TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    payload BYTEA NOT NULL,
    UNIQUE (scope, idempotency_key)
);

CREATE INDEX protocol_memory_writes_scope_idx
    ON protocol_memory_writes(scope, write_id);

CREATE TABLE protocol_execution_operations (
    operation_id TEXT PRIMARY KEY,
    payload BYTEA NOT NULL
);

CREATE TABLE protocol_execution_intents (
    operation_id TEXT NOT NULL REFERENCES protocol_execution_operations(operation_id),
    claim_fence TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (operation_id, claim_fence)
);

CREATE TABLE protocol_execution_receipts (
    operation_id TEXT PRIMARY KEY REFERENCES protocol_execution_operations(operation_id),
    receipt_id TEXT NOT NULL UNIQUE,
    observed_at TIMESTAMPTZ NOT NULL,
    payload BYTEA NOT NULL
);

CREATE TABLE protocol_budget_ledgers (
    account_id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 0)
);

CREATE TABLE protocol_budget_ledger_events (
    account_id TEXT NOT NULL REFERENCES protocol_budget_ledgers(account_id) ON DELETE CASCADE,
    version BIGINT NOT NULL CHECK (version > 0),
    payload BYTEA NOT NULL,
    PRIMARY KEY (account_id, version)
);

CREATE TABLE protocol_budget_reservations (
    reservation_id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES protocol_budget_ledgers(account_id) ON DELETE CASCADE,
    pending BOOLEAN NOT NULL,
    payload BYTEA NOT NULL
);

CREATE INDEX protocol_budget_reservations_pending_idx
    ON protocol_budget_reservations(pending, reservation_id);
