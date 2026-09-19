CREATE TABLE authorization_policy_events (
    policy_id TEXT NOT NULL,
    version BIGINT NOT NULL CHECK (version > 0),
    payload BYTEA NOT NULL,
    PRIMARY KEY (policy_id, version)
);

CREATE TABLE authorization_policy_state (
    policy_id TEXT PRIMARY KEY,
    version BIGINT NOT NULL CHECK (version >= 0),
    payload BYTEA NOT NULL
);

CREATE TABLE authorization_decisions (
    policy_id TEXT NOT NULL,
    decision_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (policy_id, decision_id),
    UNIQUE (policy_id, request_id)
);

