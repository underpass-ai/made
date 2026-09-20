-- Agentic system designs, their sealed revisions, and the runs of them.
--
-- The design table is a revision log, not a mutable row: the primary
-- key is (system_id, revision), and an edit inserts the next revision
-- rather than replacing the one it read. That is what makes a
-- concurrent edit a refused insert instead of a lost update, under
-- concurrency rather than by agreement.
CREATE TABLE agentic_systems (
    system_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    lifecycle TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (system_id, revision)
);

-- Listing the catalogue means listing heads, and a head is the largest
-- revision of each design. Descending so the first row of each design
-- is the one a listing wants.
CREATE INDEX agentic_systems_heads ON agentic_systems (system_id, revision DESC);

-- Sealed revisions, which never change once written. A separate table
-- rather than a flag on the design: a draft being edited and a
-- revision a run is pinned to are different things, and a store that
-- could not tell them apart could not promise the second is still what
-- it was.
CREATE TABLE agentic_system_publications (
    system_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    digest TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (system_id, revision)
);

-- One row per run. `updated_at` is the version a write compares
-- against, so two advances of one run cannot each overwrite the
-- other's links.
CREATE TABLE agentic_system_executions (
    execution_id TEXT PRIMARY KEY,
    system_id TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    payload BYTEA NOT NULL
);

-- "What has been run from this design" is a range scan in creation
-- order, which is the order the store hands runs back in.
CREATE INDEX agentic_system_executions_by_system
    ON agentic_system_executions (system_id, execution_id);
