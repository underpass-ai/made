CREATE TABLE artifact_uploads (
    upload_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    request JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('active', 'committed', 'aborted')),
    artifact JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE artifact_upload_chunks (
    upload_id TEXT NOT NULL REFERENCES artifact_uploads(upload_id) ON DELETE CASCADE,
    chunk_offset BIGINT NOT NULL CHECK (chunk_offset >= 0),
    bytes BYTEA NOT NULL,
    chunk_digest TEXT NOT NULL,
    PRIMARY KEY (upload_id, chunk_offset)
);

CREATE TABLE artifact_blobs (
    digest TEXT NOT NULL,
    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),
    chunk_offset BIGINT NOT NULL CHECK (chunk_offset >= 0),
    bytes BYTEA NOT NULL,
    PRIMARY KEY (digest, size_bytes, chunk_offset)
);

CREATE TABLE artifact_records (
    artifact_id TEXT PRIMARY KEY,
    body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX artifact_records_id_idx ON artifact_records (artifact_id);
