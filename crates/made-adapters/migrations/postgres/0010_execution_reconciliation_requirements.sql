CREATE TABLE ceremony_execution_reconciliation_requirements (
    operation_id TEXT NOT NULL,
    claim_fence TEXT NOT NULL,
    payload BYTEA NOT NULL,
    PRIMARY KEY (operation_id, claim_fence),
    FOREIGN KEY (operation_id, claim_fence)
        REFERENCES ceremony_execution_intents(operation_id, claim_fence)
);
