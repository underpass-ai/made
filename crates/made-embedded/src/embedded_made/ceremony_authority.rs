use std::sync::Arc;

use made_app::services::{AuthorizationOperationScope, SessionStream};
use made_app::usecases::ResolveCeremonyDefinitionUseCase;
use made_core::value_objects::{AuthorizationAction, AuthorizationScope, CeremonyId};
use made_core::DomainError;

use super::{EmbeddedCeremonyProjectionData, EmbeddedMade};

/// Internal-facing projection collaborator for an already-authorized adapter.
///
/// This keeps adapter follow-up reads out of the public facade. A protected
/// engine only projects the ceremony bound to the active typed operation and
/// only for actions whose response contract contains ceremony state.
#[derive(Clone)]
pub struct EmbeddedCeremonyAuthority {
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    protected: bool,
}

impl std::fmt::Debug for EmbeddedCeremonyAuthority {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EmbeddedCeremonyAuthority")
            .field("protected", &self.protected)
            .finish_non_exhaustive()
    }
}

impl EmbeddedCeremonyAuthority {
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
        ceremony_id: &CeremonyId,
    ) -> Result<EmbeddedCeremonyProjectionData, DomainError> {
        self.require_projection(ceremony_id)?;
        let records = self.stream.records(ceremony_id).await?;
        let read = SessionStream::fold_records(&records)?;
        let definition = self.definitions.execute(&read.instance).await?;
        Ok(EmbeddedCeremonyProjectionData::new(
            read.instance,
            definition,
            records,
        ))
    }

    fn require_projection(&self, ceremony_id: &CeremonyId) -> Result<(), DomainError> {
        if !self.protected {
            return Ok(());
        }
        let operation =
            AuthorizationOperationScope::current().ok_or(DomainError::InvariantViolated {
                reason: "protected ceremony projection requires an authorized operation context",
            })?;
        let action = operation.evidence().action();
        if !projects_ceremony_state(action) {
            return Err(DomainError::InvariantViolated {
                reason: "authorized operation does not admit a ceremony response projection",
            });
        }
        let matches = match operation.evidence().scope() {
            AuthorizationScope::Ceremony {
                ceremony_id: admitted,
            }
            | AuthorizationScope::ResolvedCeremony {
                ceremony_id: admitted,
                ..
            } => admitted == ceremony_id,
            AuthorizationScope::Global => matches!(
                action,
                AuthorizationAction::ListCeremonyInstances
                    | AuthorizationAction::SearchCeremonyInstances
            ),
            _ => false,
        };
        if matches {
            return Ok(());
        }
        Err(DomainError::InvariantViolated {
            reason: "authorized operation scope does not admit this ceremony projection",
        })
    }
}

fn projects_ceremony_state(action: AuthorizationAction) -> bool {
    use AuthorizationAction as A;
    matches!(
        action,
        A::GetCeremonyInstance
            | A::ListCeremonyInstances
            | A::SearchCeremonyInstances
            | A::GenerateCeremonyReport
            | A::ReadCeremonyEvents
            | A::StreamCeremony
            | A::PullCeremonyEvents
            | A::VerifyCeremonyJournal
            | A::GetCeremonyTranscript
            | A::RunCeremony
            | A::StartCeremony
            | A::StartPublishedCeremony
            | A::RunCeremonyStep
            | A::ClaimCeremonyStep
            | A::CompleteCeremonyStep
            | A::CompleteExecutionReceipt
            | A::AdoptExecutionReceipt
            | A::PrepareCeremonyChildren
            | A::AcceptChildCompletion
            | A::RecoverCeremonyChildren
            | A::ApplyCeremonyTransition
            | A::EnforceCeremonyDeadlines
            | A::BindCeremonyParticipants
            | A::PauseCeremony
            | A::ResumeCeremony
            | A::CancelCeremony
            | A::ApproveCeremonyGuard
            | A::DeferCeremonyGuard
            | A::RequestCeremonyIntervention
            | A::RespondToCeremonyIntervention
            | A::CloseCeremonyIntervention
            | A::PullCeremonyAgentInterventions
            | A::AcknowledgeCeremonyAgentIntervention
            | A::GetCeremonyIntervention
            | A::ListCeremonyInterventions
            | A::CollectCeremonyEvidence
            | A::AssertCeremonyReason
    )
}
