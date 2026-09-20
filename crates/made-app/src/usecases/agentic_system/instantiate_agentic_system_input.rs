use std::collections::BTreeMap;

use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AgenticSystemRevision, AuditActorId, AuditActorKind,
    CeremonyContext, HostDestination, ParticipantId, SystemCeremonyId,
};

use super::ParticipantOffer;

/// A request to run one sealed revision of a design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiateAgenticSystemInput {
    system_id: AgenticSystemId,
    revision: AgenticSystemRevision,
    /// The caller's own name for this run, and its idempotency key.
    ///
    /// Retrying with the same one answers with the run that exists.
    /// A host that did not hear the first answer must be able to ask
    /// again without starting the work twice.
    execution_id: AgenticSystemExecutionId,
    inputs: BTreeMap<SystemCeremonyId, CeremonyContext>,
    offers: BTreeMap<ParticipantId, ParticipantOffer>,
    integrator_destination: Option<HostDestination>,
    actor_id: AuditActorId,
    actor_kind: AuditActorKind,
}

impl InstantiateAgenticSystemInput {
    #[must_use]
    pub fn new(
        system_id: AgenticSystemId,
        revision: AgenticSystemRevision,
        execution_id: AgenticSystemExecutionId,
        inputs: impl IntoIterator<Item = (SystemCeremonyId, CeremonyContext)>,
        offers: impl IntoIterator<Item = (ParticipantId, ParticipantOffer)>,
        integrator_destination: Option<HostDestination>,
        actor_id: impl Into<String>,
        actor_kind: AuditActorKind,
    ) -> Self {
        Self {
            system_id,
            revision,
            execution_id,
            inputs: inputs.into_iter().collect(),
            offers: offers.into_iter().collect(),
            integrator_destination,
            actor_id: AuditActorId::new(actor_id),
            actor_kind,
        }
    }

    #[must_use]
    pub const fn system_id(&self) -> &AgenticSystemId {
        &self.system_id
    }

    #[must_use]
    pub const fn revision(&self) -> AgenticSystemRevision {
        self.revision
    }

    #[must_use]
    pub const fn execution_id(&self) -> &AgenticSystemExecutionId {
        &self.execution_id
    }

    #[must_use]
    pub const fn inputs(&self) -> &BTreeMap<SystemCeremonyId, CeremonyContext> {
        &self.inputs
    }

    /// What the host says it can supply for each logical participant.
    #[must_use]
    pub const fn offers(&self) -> &BTreeMap<ParticipantId, ParticipantOffer> {
        &self.offers
    }

    #[must_use]
    pub const fn integrator_destination(&self) -> Option<&HostDestination> {
        self.integrator_destination.as_ref()
    }

    #[must_use]
    pub const fn actor_id(&self) -> &AuditActorId {
        &self.actor_id
    }

    #[must_use]
    pub const fn actor_kind(&self) -> AuditActorKind {
        self.actor_kind
    }
}
