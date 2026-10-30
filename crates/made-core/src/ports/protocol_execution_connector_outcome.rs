use crate::value_objects::ExecutionOperationId;

use super::ProtocolExecutionObservation;

/// Authoritative observation or an operation that now needs reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolExecutionConnectorOutcome {
    Observed(Box<ProtocolExecutionObservation>),
    ReconciliationRequired(ExecutionOperationId),
}
