CREATE TABLE artifact_protections (
    protection_key TEXT PRIMARY KEY,
    body JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('protected', 'released')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX artifact_protections_live_idx
    ON artifact_protections (protection_key)
    WHERE state = 'protected';
