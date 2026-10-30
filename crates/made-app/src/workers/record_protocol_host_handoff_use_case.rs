use super::RecordProtocolHostHandoffInput;
use crate::authorization::{AuthorizationGateOutcome, AuthorizeOperationUseCase};
use crate::services::{session_facts, AuthorizationOperationScope, ConflictPolicy, SessionStream};
use crate::usecases::ResolveProtocolDefinitionUseCase;
use made_core::entities::protocol_commands::RecordHostHandoff;
use made_core::entities::protocol_events::HostHandoffRecorded;
use made_core::entities::{AuditRecord, ProtocolCommand, ProtocolEvent};
use made_core::ports::ClockPort;
use made_core::value_objects::{AuditActorKind, AuthorizationRequestId};
use made_core::DomainError;
use std::sync::Arc;

pub struct RecordProtocolHostHandoffUseCase {
    stream: Arc<SessionStream>,
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    clock: Arc<dyn ClockPort>,
    authorization: Option<Arc<AuthorizeOperationUseCase>>,
}

impl std::fmt::Debug for RecordProtocolHostHandoffUseCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordProtocolHostHandoffUseCase")
            .finish_non_exhaustive()
    }
}

impl RecordProtocolHostHandoffUseCase {
    #[must_use]
    pub fn new(
        stream: Arc<SessionStream>,
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            stream,
            definitions,
            clock,
            authorization: None,
        }
    }

    #[must_use]
    pub fn with_reauthorization(mut self, authorization: Arc<AuthorizeOperationUseCase>) -> Self {
        self.authorization = Some(authorization);
        self
    }

    pub async fn execute(
        &self,
        input: RecordProtocolHostHandoffInput,
    ) -> Result<HostHandoffRecorded, DomainError> {
        input.declaration.validate()?;
        if let Some(operation) = AuthorizationOperationScope::current() {
            let records = self.stream.records(&input.protocol_id).await?;
            let accepted_started = records.iter().find(|record| matches!(record.event(),
                Some(ProtocolEvent::StepStarted(started))
                    if started.step_id == input.declaration.step_id
                    && started.lease.owner_id() == &input.declaration.owner
                    && started.claim_fence(&input.protocol_id).is_ok_and(|fence| fence == input.declaration.claim_fence)
            ));
            let principal_owns_claim =
                accepted_started.is_some_and(|source| {
                    source.authorization_evidence().is_some_and(|evidence| {
                        evidence.principal_id() == operation.principal().id()
                    })
                }) || imported_claim_owned_by_principal(&records, &input, &operation);
            if !principal_owns_claim {
                return Err(DomainError::InvariantViolated {
                    reason: "handoff principal does not own the accepted claim",
                });
            }
            let authorize = self
                .authorization
                .as_ref()
                .ok_or(DomainError::InvariantViolated {
                    reason: "protected host handoff requires current authorization",
                })?;
            let check =
                AuthorizationRequestId::new(format!("handoff-check:{}", uuid::Uuid::new_v4()))?;
            if !matches!(
                authorize.revalidate(&operation, check).await?,
                AuthorizationGateOutcome::Allowed { .. }
            ) {
                return Err(DomainError::InvariantViolated {
                    reason: "current authorization refuses host handoff",
                });
            }
        }
        let session = self.stream.load(&input.protocol_id).await?;
        let definition = self.definitions.execute(&session.instance).await?;
        let updated = self
            .stream
            .execute(session, ConflictPolicy::retry(), |session| {
                let now = self.clock.now();
                let events = session.instance.decide(
                    &ProtocolCommand::RecordHostHandoff(RecordHostHandoff {
                        declaration: input.declaration.clone(),
                        now,
                    }),
                    &definition,
                )?;
                let actor = session_facts::party(
                    input.declaration.owner.as_str(),
                    AuditActorKind::Service,
                )?;
                session_facts::facts(&session.instance, events, &actor, now)
            })
            .await?;
        updated
            .instance
            .host_handoffs()
            .get(&input.declaration.id)
            .cloned()
            .ok_or(DomainError::NotFound {
                what: "recorded_host_handoff",
            })
    }
}

/// A pre-stream import seals the only trustworthy provenance available for a
/// legacy claim. It has no `StepStarted` or contemporary authorization
/// evidence, so accept it only when the authenticated principal explicitly
/// names the imported lease owner and every immutable claim coordinate agrees.
fn imported_claim_owned_by_principal(
    records: &[AuditRecord],
    input: &RecordProtocolHostHandoffInput,
    operation: &made_core::value_objects::AuthorizedOperation,
) -> bool {
    operation.principal().id().as_str() == input.declaration.owner.as_str()
        && records.iter().any(|record| {
            let Some(ProtocolEvent::InstanceImported(imported)) = record.event() else {
                return false;
            };
            let Some(step) = imported.snapshot.step_record(&input.declaration.step_id) else {
                return false;
            };
            step.status() == made_core::value_objects::StepStatus::InProgress
                && step
                    .lease()
                    .is_some_and(|lease| lease.owner_id() == &input.declaration.owner)
                && imported
                    .snapshot
                    .step_claim_fence(&input.declaration.step_id)
                    .is_ok_and(|fence| fence == input.declaration.claim_fence)
        })
}
