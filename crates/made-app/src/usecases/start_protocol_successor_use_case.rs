//! [`StartProtocolSuccessorUseCase`] — seal the handoff, then open the
//! successor.
//!
//! Two appends to two streams, in that order, and the order is the
//! whole design. The predecessor is sealed first, so a crash in
//! between leaves a protocol that says exactly what it intended and an
//! opening that can be made or verified. Opening first would leave a
//! stream with no recorded provenance, indistinguishable from an
//! unrelated instance that happens to hold that id.
//!
//! Everything the second append writes is derived from what the first
//! one sealed — the successor's id, the moment it opened, the evidence
//! it starts with — so a retry builds the same batch the first attempt
//! built. That is what lets the retry verify an existing opening
//! instead of guessing whether it made it.

use std::sync::Arc;

use made_core::entities::protocol_commands::PlanSuccessor;
use made_core::entities::{
    ProtocolCommand, ProtocolEvent, ProtocolInstance, PublishedProtocolDefinition,
};
use made_core::error::DomainError;
use made_core::ports::{ClockPort, ProtocolDefinitionPublicationPort};
use made_core::value_objects::{
    AuditActorKind, AuthorizationAction, AuthorizationRequest, AuthorizationRequestId,
    AuthorizationScope, AuthorizationTargetDigest, CarriedEvidence, DefinitionPin,
    ProtocolDefinitionDiff, ProtocolName, ProtocolSuccession, ProtocolVersion, SuccessionPlan,
    SuccessorProtocolId,
};

mod support;
#[cfg(test)]
mod tests;

use support::{carried_evidence, predecessor_cut, sealed_plan, verify_existing_opening};

use super::{
    ProtocolSuccessorOutcome, ResolveProtocolDefinitionUseCase, StartProtocolSuccessorInput,
};
use crate::authorization::{AuthorizationGateOutcome, AuthorizeOperationUseCase};
use crate::services::{session_facts, AuthorizationOperationScope, ConflictPolicy, SessionStream};

/// Who the engine says opened the successor.
///
/// The person who decided is recorded in the plan; this names the part
/// the engine played, which is opening a stream a sealed fact told it
/// to open. Fixed rather than taken from the caller so the retry that
/// verifies an opening does not disagree with the attempt that made it.
const OPENER: &str = "made-successor-opener";

pub struct StartProtocolSuccessorUseCase {
    definitions: Arc<ResolveProtocolDefinitionUseCase>,
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
    stream: Arc<SessionStream>,
    clock: Arc<dyn ClockPort>,
    /// Configured wherever the surface is protected. Planning a
    /// successor does not grant the right to start one, and the right
    /// to end one protocol is not the right to open a session under
    /// somebody else's definition, so the target definition is admitted
    /// as its own question.
    reauthorize: Option<Arc<AuthorizeOperationUseCase>>,
}

impl std::fmt::Debug for StartProtocolSuccessorUseCase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StartProtocolSuccessorUseCase")
            .finish_non_exhaustive()
    }
}

impl StartProtocolSuccessorUseCase {
    #[must_use]
    pub fn new(
        definitions: Arc<ResolveProtocolDefinitionUseCase>,
        publications: Arc<dyn ProtocolDefinitionPublicationPort>,
        stream: Arc<SessionStream>,
        clock: Arc<dyn ClockPort>,
    ) -> Self {
        Self {
            definitions,
            publications,
            stream,
            clock,
            reauthorize: None,
        }
    }

    /// Admit the successor's definition separately, against the same
    /// policy and the same principal as the operation in flight.
    #[must_use]
    pub fn with_reauthorization(mut self, authorize: Arc<AuthorizeOperationUseCase>) -> Self {
        self.reauthorize = Some(authorize);
        self
    }

    pub async fn execute(
        &self,
        input: StartProtocolSuccessorInput,
    ) -> Result<ProtocolSuccessorOutcome, DomainError> {
        let records = self.stream.records(&input.instance_id).await?;
        let session = SessionStream::fold_records(&records)?;
        let (definition, predecessor_definition) =
            self.definitions.execute_pinned(&session.instance).await?;
        self.require_definition_admitted(&input.definition_name, &input.definition_version)
            .await?;
        let successor = self.publication(&input).await?;
        let diff = ProtocolDefinitionDiff::between(&definition, successor.definition());
        let plan = self.plan(&input, &session.instance, &records, &successor)?;

        let command = ProtocolCommand::PlanSuccessor(PlanSuccessor {
            plan: plan.clone(),
            successor: Box::new(successor.clone()),
            diff,
            now: plan.planned_at(),
        });
        let actor = session_facts::party(input.actor_id.as_str(), input.actor_kind)?;
        let sealed = self
            .stream
            .execute(session, ConflictPolicy::retry(), |current| {
                let events = current.instance.decide(&command, &definition)?;
                session_facts::facts(&current.instance, events, &actor, plan.planned_at())
            })
            .await?;

        let succession = self
            .succession(&input, &plan, predecessor_definition, &successor)
            .await?;
        let context = input
            .context_overrides
            .unwrap_or_else(|| sealed.instance.context().clone());
        let opening = ProtocolInstance::decide_start_successor(
            plan.successor_id().clone(),
            &successor,
            context,
            succession,
            &plan,
            plan.planned_at(),
        )?;
        let successor_instance = self
            .open_or_verify(plan.successor_id(), &plan, opening)
            .await?;
        Ok(ProtocolSuccessorOutcome {
            successor: successor_instance,
            plan,
        })
    }

    /// Whether this caller may open a session under the successor's
    /// definition.
    ///
    /// The operation in flight is scoped to the protocol handing off,
    /// so it cannot also carry a definition scope: this is a second
    /// admission, decided against the same policy and the same
    /// principal. Where nothing protects the surface there is no
    /// operation to decide against and nothing to ask.
    async fn require_definition_admitted(
        &self,
        name: &ProtocolName,
        version: &ProtocolVersion,
    ) -> Result<(), DomainError> {
        let Some(authorize) = &self.reauthorize else {
            return Ok(());
        };
        let Some(operation) = AuthorizationOperationScope::current() else {
            return Ok(());
        };
        let target = format!("{}@{}", name.as_str(), version.as_str());
        let request = AuthorizationRequest::new(
            AuthorizationRequestId::new(format!("successor-definition:{target}"))?,
            operation.principal().clone(),
            AuthorizationAction::StartPublishedProtocol,
            AuthorizationScope::Definition {
                name: name.clone(),
                version: Some(version.clone()),
            },
            AuthorizationTargetDigest::for_bytes(target.as_bytes()),
        );
        match authorize.execute(request).await? {
            AuthorizationGateOutcome::Allowed { .. } => Ok(()),
            AuthorizationGateOutcome::Denied { .. } | AuthorizationGateOutcome::Expired { .. } => {
                Err(DomainError::InvariantViolated {
                    reason: "current authorization does not admit the successor's definition",
                })
            }
        }
    }

    async fn publication(
        &self,
        input: &StartProtocolSuccessorInput,
    ) -> Result<PublishedProtocolDefinition, DomainError> {
        self.publications
            .published(&input.definition_name, &input.definition_version)
            .await?
            .ok_or(DomainError::NotFound {
                what: "published_protocol_definition",
            })
    }

    /// The plan this call means, whether it is being sealed now or was
    /// sealed before.
    ///
    /// A handoff already sealed under this plan id is the truth, and
    /// its own moment is what the candidate is built with, so a retry
    /// of the same request produces the same plan rather than a second
    /// one stamped with a later clock. A retry that asks for something
    /// else is a different handoff wearing one name, and the decision
    /// refuses it.
    fn plan(
        &self,
        input: &StartProtocolSuccessorInput,
        instance: &ProtocolInstance,
        records: &[made_core::entities::AuditRecord],
        successor: &PublishedProtocolDefinition,
    ) -> Result<SuccessionPlan, DomainError> {
        let successor_id =
            SuccessorProtocolId::derive(instance.id(), &input.plan_id)?.into_protocol_id();
        let planned_at = sealed_plan(instance, &input.plan_id)
            .map_or_else(|| self.clock.now(), SuccessionPlan::planned_at);
        let carried: Vec<CarriedEvidence> = carried_evidence(instance, records, &input.carried)?;
        Ok(SuccessionPlan::new(
            input.plan_id.clone(),
            successor_id,
            DefinitionPin::new(
                successor.name().clone(),
                successor.version().clone(),
                successor.digest(),
            ),
            carried,
            input.dispositions.clone(),
            input.budget,
            input.actor_id.clone(),
            planned_at,
        ))
    }

    /// Which cut of the predecessor this successor was planned against.
    ///
    /// Read off the record that sealed the plan rather than off the
    /// stream's head: the head can move on — a superseded protocol can
    /// still be cancelled — and a successor whose provenance depended
    /// on that would open differently on a retry.
    async fn succession(
        &self,
        input: &StartProtocolSuccessorInput,
        plan: &SuccessionPlan,
        predecessor_definition: DefinitionPin,
        successor: &PublishedProtocolDefinition,
    ) -> Result<ProtocolSuccession, DomainError> {
        let records = self.stream.records(&input.instance_id).await?;
        let (head, version) = predecessor_cut(&records, plan.plan_id())?;
        Ok(ProtocolSuccession::new(
            input.instance_id.clone(),
            head,
            version,
            predecessor_definition,
            DefinitionPin::new(
                successor.name().clone(),
                successor.version().clone(),
                successor.digest(),
            ),
            plan.plan_id().clone(),
        ))
    }

    /// Open the successor's stream, or check that the one already
    /// there is the one this plan opens.
    ///
    /// The expected-empty append is what makes the opening atomic. A
    /// stream that exists holding something else is a foreign protocol
    /// and is refused; a stream that exists holding exactly this is the
    /// retry landing on its own earlier work.
    async fn open_or_verify(
        &self,
        successor_id: &made_core::value_objects::ProtocolId,
        plan: &SuccessionPlan,
        opening: Vec<ProtocolEvent>,
    ) -> Result<ProtocolInstance, DomainError> {
        let actor = session_facts::party(OPENER, AuditActorKind::Service)?;
        match self
            .stream
            .open(opening.clone(), actor, plan.planned_at())
            .await
        {
            Ok(session) => Ok(session.instance),
            Err(DomainError::AlreadyExists { .. }) => {
                let records = self.stream.records(successor_id).await?;
                verify_existing_opening(&records, &opening)?;
                Ok(SessionStream::fold_records(&records)?.instance)
            }
            Err(error) => Err(error),
        }
    }
}
