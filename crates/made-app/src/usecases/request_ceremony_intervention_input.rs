use made_core::value_objects::{
    AuditActorKind, CeremonyId, CeremonyInterventionContent, CeremonyInterventionId,
    CeremonyInterventionIntent, CeremonyInterventionKind, CeremonyInterventionProvenance,
    CeremonyInterventionTarget, InterventionDeliveryPolicy, SupervisorPrincipal,
    RoleId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestCeremonyInterventionInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) intervention_id: CeremonyInterventionId,
    pub(crate) role_id: RoleId,
    /// What kind of party fills that seat.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) kind: CeremonyInterventionKind,
    pub(crate) target: CeremonyInterventionTarget,
    pub(crate) content: CeremonyInterventionContent,
    pub(crate) provenance: Option<CeremonyInterventionProvenance>,
    pub(crate) intent: Option<CeremonyInterventionIntent>,
    pub(crate) delivery: Option<InterventionDeliveryPolicy>,
    pub(crate) supervisor: Option<SupervisorPrincipal>,
}

impl RequestCeremonyInterventionInput {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        intervention_id: CeremonyInterventionId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        kind: CeremonyInterventionKind,
        target: CeremonyInterventionTarget,
        content: CeremonyInterventionContent,
    ) -> Self {
        Self {
            instance_id,
            intervention_id,
            role_id,
            role_kind,
            kind,
            target,
            content,
            provenance: None,
            intent: None,
            delivery: None,
            supervisor: None,
        }
    }

    #[must_use]
    pub fn with_intent(mut self, intent: CeremonyInterventionIntent) -> Self {
        self.intent = Some(intent);
        self
    }

    #[must_use]
    pub fn with_delivery(mut self, delivery: InterventionDeliveryPolicy) -> Self {
        self.delivery = Some(delivery);
        self
    }

    /// Ask on behalf of somebody who holds no seat at the table.
    ///
    /// The seat is replaced by the one derived from the principal, so
    /// a caller cannot pass a declared role here and inherit what that
    /// role may do. The use case is what checks that the ambient
    /// authorization actually names this principal.
    pub fn asked_by_supervisor(
        mut self,
        supervisor: SupervisorPrincipal,
    ) -> Result<Self, made_core::error::DomainError> {
        self.role_id = supervisor.requesting_role()?;
        self.role_kind = AuditActorKind::Human;
        self.supervisor = Some(supervisor);
        Ok(self)
    }

    #[must_use]
    pub const fn supervisor(&self) -> Option<&SupervisorPrincipal> {
        self.supervisor.as_ref()
    }

    #[must_use]
    pub fn with_provenance(mut self, provenance: CeremonyInterventionProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }
    #[must_use]
    pub fn intervention_id(&self) -> &CeremonyInterventionId {
        &self.intervention_id
    }

    #[must_use]
    pub fn target(&self) -> &CeremonyInterventionTarget {
        &self.target
    }

    #[must_use]
    pub const fn kind(&self) -> CeremonyInterventionKind {
        self.kind
    }

    #[must_use]
    pub const fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
