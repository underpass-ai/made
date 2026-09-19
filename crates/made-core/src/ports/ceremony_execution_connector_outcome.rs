use crate::value_objects::ExecutionOperationId;

use super::CeremonyExecutionObservation;

/// Authoritative observation or an operation that now needs reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyExecutionConnectorOutcome {
    Observed(Box<CeremonyExecutionObservation>),
    ReconciliationRequired(ExecutionOperationId),
}
