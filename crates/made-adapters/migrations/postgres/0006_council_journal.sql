-- This order and its consumer namespace are independent of ceremony feeds.
-- A transactional counter, rather than a sequence, preserves commit order.
CREATE TABLE council_journal_meta (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    last_position BIGINT NOT NULL CHECK (last_position >= 0)
);
INSERT INTO council_journal_meta (singleton, last_position) VALUES (TRUE, 0);
CREATE TABLE council_journal (
    position BIGINT PRIMARY KEY CHECK (position > 0),
    publication_id TEXT UNIQUE,
    record JSONB NOT NULL
);
CREATE TABLE council_journal_cursors (
    consumer TEXT PRIMARY KEY,
    state JSONB NOT NULL
);
CREATE TABLE council_contracts (
    contract_id TEXT PRIMARY KEY,
    body JSONB NOT NULL
);
CREATE TABLE council_snapshot_imports (
    source TEXT PRIMARY KEY,
    snapshot JSONB NOT NULL,
    record JSONB NOT NULL
);
