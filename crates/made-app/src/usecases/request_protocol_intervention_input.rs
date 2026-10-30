use made_core::value_objects::{
    AuditActorKind, InterventionDeliveryPolicy, ProtocolId, ProtocolInterventionContent,
    ProtocolInterventionId, ProtocolInterventionIntent, ProtocolInterventionKind,
    ProtocolInterventionProvenance, ProtocolInterventionTarget, RoleId, SupervisorPrincipal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestProtocolInterventionInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) intervention_id: ProtocolInterventionId,
    pub(crate) role_id: RoleId,
    /// What kind of party fills that seat.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) kind: ProtocolInterventionKind,
    pub(crate) target: ProtocolInterventionTarget,
    pub(crate) content: ProtocolInterventionContent,
    pub(crate) provenance: Option<ProtocolInterventionProvenance>,
    pub(crate) intent: Option<ProtocolInterventionIntent>,
    pub(crate) delivery: Option<InterventionDeliveryPolicy>,
    pub(crate) supervisor: Option<SupervisorPrincipal>,
}

impl RequestProtocolInterventionInput {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        intervention_id: ProtocolInterventionId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        kind: ProtocolInterventionKind,
        target: ProtocolInterventionTarget,
        content: ProtocolInterventionContent,
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
    pub fn with_intent(mut self, intent: ProtocolInterventionIntent) -> Self {
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
    pub fn with_provenance(mut self, provenance: ProtocolInterventionProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }
    #[must_use]
    pub fn intervention_id(&self) -> &ProtocolInterventionId {
        &self.intervention_id
    }

    #[must_use]
    pub fn target(&self) -> &ProtocolInterventionTarget {
        &self.target
    }

    #[must_use]
    pub const fn kind(&self) -> ProtocolInterventionKind {
        self.kind
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
