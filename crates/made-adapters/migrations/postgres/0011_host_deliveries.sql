-- Work handed out to hosts, and who is driving each ceremony.
--
-- One row per delivery, keyed by the identity the domain derives from
-- what is being delivered and where: the same item offered to the same
-- destination twice is one row, and the primary key is what makes that
-- true under concurrency rather than by agreement.
CREATE TABLE host_deliveries (
    delivery_id TEXT PRIMARY KEY,
    ceremony_id TEXT NOT NULL,
    target_key TEXT NOT NULL,
    state TEXT NOT NULL,
    payload BYTEA NOT NULL
);

-- A host asks "what is waiting for me", and an operator asks "what is
-- outstanding in this ceremony". Both are range scans in identifier
-- order, which is also the order the ledger pages in.
CREATE INDEX host_deliveries_by_target ON host_deliveries (target_key, delivery_id);
CREATE INDEX host_deliveries_by_ceremony ON host_deliveries (ceremony_id, delivery_id);
CREATE INDEX host_deliveries_by_state ON host_deliveries (state, delivery_id);

-- One row per scope, holding every integrator ever bound to it. A
-- revoked binding stays: who was driving when a ceremony went quiet is
-- a question asked afterwards.
CREATE TABLE integrator_bindings (
    scope_key TEXT PRIMARY KEY,
    payload BYTEA NOT NULL
);
