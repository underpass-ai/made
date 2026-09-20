//! [`AcknowledgeCeremonyAgentInterventionUseCase`] — a host says what
//! it saw, and the ceremony seals it.

use std::sync::Arc;

use made_core::entities::ceremony_commands::AcknowledgeInterventionDelivery;
use made_core::entities::{CeremonyCommand, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{AckOutcome, ClockPort, DeliveryFailureOutcome, HostDeliveryLedgerPort};
use made_core::value_objects::{AuditActorKind, DeliveryFailureReason, InterventionDeliveryAck};
use time::OffsetDateTime;

use super::acknowledge_ceremony_agent_intervention_input::AcknowledgeCeremonyAgentInterventionInput;
use super::resolve_ceremony_definition_use_case::ResolveCeremonyDefinitionUseCase;
use crate::services::{session_facts, ConflictPolicy, SessionStream};

pub struct AcknowledgeCeremonyAgentInterventionUseCase {
    definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    deliveries: Arc<dyn HostDeliveryLedgerPort>,
    clock: Arc<dyn ClockPort>,
}

impl std::fmt::Debug for AcknowledgeCeremonyAgentInterventionUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcknowledgeCeremonyAgentInterventionUseCase")
            .finish_non_exhaustive()
    }
}

impl AcknowledgeCeremonyAgentInterventionUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveCeremonyDefinitionUseCase>,
        stream: Arc<SessionStream>,
        deliveries: Arc<dyn HostDeliveryLedgerPort>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            stream,
            deliveries,
            clock,
        }
    }

    #[tracing::instrument(
        name = "acknowledge_ceremony_agent_intervention",
        skip_all,
        fields(
            ceremony_id = %input.instance_id,
            intervention_id = %input.intervention_id,
            delivery_id = %input.lease.delivery_id(),
        )
    )]
    pub async fn execute(
        &self,
        input: AcknowledgeCeremonyAgentInterventionInput,
    ) -> Result<CeremonyInstance, DomainError> {
        let now = self.clock.now();
        // A host that is busy or silent has not seen the item, so
        // there is nothing to seal about it: the attempt is counted
        // and the offer goes back for whoever can take it, or runs out
        // of attempts and stays visibly failed. Only a host that took
        // the item — including one that took it and refused — has made
        // an observation worth a place in the ceremony's stream.
        if !self.is_an_observation(&input) {
            return self.count_a_failed_attempt(&input, now).await;
        }
        // The ledger goes first because it owns the lease, and a lease
        // that is not this host's is the one refusal that must reach
        // the caller before anything is sealed about it.
        let outcome = self
            .deliveries
            .acknowledge(&input.lease, &input.observation, now)
            .await?;
        match outcome {
            AckOutcome::Acknowledged(_) | AckOutcome::AlreadyAcknowledged(_) => {}
            AckOutcome::Conflict { .. } => {
                return Err(DomainError::Conflict {
                    what: "observation already recorded for this host delivery",
                })
            }
            AckOutcome::LeaseNotOwned => {
                return Err(DomainError::Conflict {
                    what: "live lease on this host delivery",
                })
            }
        }

        let session = self.stream.load(&input.instance_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let actor = session_facts::seat(input.recipient.role_id(), AuditActorKind::Agent)?;
        let ack = InterventionDeliveryAck::new(
            input.lease.delivery_id().clone(),
            input.recipient.clone(),
            input.observation.clone(),
            now,
        );
        let command =
            CeremonyCommand::AcknowledgeInterventionDelivery(AcknowledgeInterventionDelivery {
                intervention_id: input.intervention_id.clone(),
                ack,
                now,
            });
        let instance = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let events = session.instance.decide(&command, &definition)?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?
            .instance;

        Ok(instance)
    }

    /// Whether the host is reporting on the item or on itself.
    ///
    /// A refusal and an incapacity are reports on the item: the host
    /// has it and will not act, which is something the ceremony needs
    /// sealed. Busy and timeout are reports on the host, and the item
    /// is still nobody's.
    fn is_an_observation(&self, input: &AcknowledgeCeremonyAgentInterventionInput) -> bool {
        !input.observation.kind().is_retryable()
    }

    async fn count_a_failed_attempt(
        &self,
        input: &AcknowledgeCeremonyAgentInterventionInput,
        now: OffsetDateTime,
    ) -> Result<CeremonyInstance, DomainError> {
        let reason = DeliveryFailureReason::new(format!(
            "host reported {}: {}",
            input.observation.kind(),
            input.observation.note()
        ))?;
        match self
            .deliveries
            .mark_failed(&input.lease, &reason, now)
            .await?
        {
            DeliveryFailureOutcome::Requeued(_) | DeliveryFailureOutcome::Exhausted(_) => {}
            DeliveryFailureOutcome::LeaseNotOwned => {
                return Err(DomainError::Conflict {
                    what: "live lease on this host delivery",
                })
            }
        }
        Ok(self.stream.load(&input.instance_id).await?.instance)
    }
}
