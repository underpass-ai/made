CREATE TABLE protocol_execution_reconciliation_requirements (
    operation_id TEXT NOT NULL,
    claim_fence TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (operation_id, claim_fence),
    FOREIGN KEY (operation_id, claim_fence)
        REFERENCES protocol_execution_intents(operation_id, claim_fence)
);
