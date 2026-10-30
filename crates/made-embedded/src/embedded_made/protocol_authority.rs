use std::sync::Arc;

use made_app::services::{AuthorizationOperationScope, SessionStream};
use made_app::usecases::ResolveProtocolDefinitionUseCase;
use made_core::value_objects::{AuthorizationAction, AuthorizationScope, ProtocolId};
use made_core::DomainError;

use super::{EmbeddedMade, EmbeddedProtocolProjectionData};

/// Internal-facing projection collaborator for an already-authorized adapter.
///
/// This keeps adapter follow-up reads out of the public facade. A protected
/// engine only projects the protocol bound to the active typed operation and
/// only for actions whose response contract contains protocol state.
#[derive(Clone)]
pub struct EmbeddedProtocolAuthority {
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    protected: bool,
}

impl std::fmt::Debug for EmbeddedProtocolAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmbeddedProtocolAuthority")
            .field("protected", &self.protected)
            .finish_non_exhaustive()
    }
}

impl EmbeddedProtocolAuthority {
    #[must_use]
    pub fn for_engine(engine: &EmbeddedMade) -> Self {
        Self {
            stream: engine.stream.clone(),
            definitions: engine.resolve_definition(),
            protected: engine.authorization.is_some(),
        }
    }

    pub async fn projection(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<EmbeddedProtocolProjectionData, DomainError> {
        self.require_projection(protocol_id)?;
        let records = self.stream.records(protocol_id).await?;
        let read = SessionStream::fold_records(&records)?;
        let definition = self.definitions.execute(&read.instance).await?;
        Ok(EmbeddedProtocolProjectionData::new(
            read.instance,
            definition,
            records,
        ))
    }

    fn require_projection(&self, protocol_id: &ProtocolId) -> Result<(), DomainError> {
        if !self.protected {
            return Ok(());
        }
        let operation =
            AuthorizationOperationScope::current().ok_or(DomainError::InvariantViolated {
                reason: "protected protocol projection requires an authorized operation context",
            })?;
        let action = operation.evidence().action();
        if !projects_protocol_state(action) {
            return Err(DomainError::InvariantViolated {
                reason: "authorized operation does not admit a protocol response projection",
            });
        }
        let matches = match operation.evidence().scope() {
            AuthorizationScope::Protocol {
                protocol_id: admitted,
            }
            | AuthorizationScope::ResolvedProtocol {
                protocol_id: admitted,
                ..
            } => admitted == protocol_id,
            AuthorizationScope::Global => matches!(
                action,
                AuthorizationAction::ListProtocolInstances
                    | AuthorizationAction::SearchProtocolInstances
            ),
            _ => false,
        };
        if matches {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this protocol projection",
        })
    }
}

fn projects_protocol_state(action: AuthorizationAction) -> bool {
    use AuthorizationAction as A;
    matches!(
        action,
        A::GetProtocolInstance
            | A::ListProtocolInstances
            | A::SearchProtocolInstances
            | A::GenerateProtocolReport
            | A::ReadProtocolEvents
            | A::StreamProtocol
            | A::PullProtocolEvents
            | A::VerifyProtocolJournal
            | A::GetProtocolTranscript
            | A::RunProtocol
            | A::StartProtocol
            | A::StartPublishedProtocol
            | A::RunProtocolStep
            | A::ClaimProtocolStep
            | A::CompleteProtocolStep
            | A::CompleteExecutionReceipt
            | A::AdoptExecutionReceipt
            | A::PrepareProtocolChildren
            | A::AcceptChildCompletion
            | A::RecoverProtocolChildren
            | A::ApplyProtocolTransition
            | A::EnforceProtocolDeadlines
            | A::BindProtocolParticipants
            | A::PauseProtocol
            | A::ResumeProtocol
            | A::CancelProtocol
            | A::ApproveProtocolGuard
            | A::DeferProtocolGuard
            | A::RequestProtocolIntervention
            | A::RespondToProtocolIntervention
            | A::CloseProtocolIntervention
            | A::PullProtocolAgentInterventions
            | A::AcknowledgeProtocolAgentIntervention
            | A::GetProtocolIntervention
            | A::ListProtocolInterventions
            | A::CollectProtocolEvidence
            | A::AssertProtocolReason
    )
}
