use made_core::value_objects::{
    AuditActorKind, CeremonyId, CeremonyInterventionContent, CeremonyInterventionId,
    DeliveryRecipient, HostDeliveryId, RoleId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespondToCeremonyInterventionInput {
    pub(crate) instance_id: CeremonyId,
    pub(crate) intervention_id: CeremonyInterventionId,
    pub(crate) role_id: RoleId,
    /// What kind of party fills that seat.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) content: CeremonyInterventionContent,
    pub(crate) recipient: Option<DeliveryRecipient>,
    pub(crate) delivery_id: Option<HostDeliveryId>,
}

impl RespondToCeremonyInterventionInput {
    #[must_use]
    pub fn new(
        instance_id: CeremonyId,
        intervention_id: CeremonyInterventionId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        content: CeremonyInterventionContent,
    ) -> Self {
        Self {
            instance_id,
            intervention_id,
            role_id,
            role_kind,
            content,
            recipient: None,
            delivery_id: None,
        }
    }

    /// Answer as the agent that was handed the item.
    ///
    /// Both halves together or neither: an answer that names its
    /// agent but not the offer it answers cannot be checked against
    /// the ledger, and one that names the offer but not the agent
    /// cannot be checked against the item's target.
    #[must_use]
    pub fn answering_delivery(
        mut self,
        recipient: DeliveryRecipient,
        delivery_id: HostDeliveryId,
    ) -> Self {
        self.recipient = Some(recipient);
        self.delivery_id = Some(delivery_id);
        self
    }

    #[must_use]
    pub const fn delivery_id(&self) -> Option<&HostDeliveryId> {
        self.delivery_id.as_ref()
    }

    #[must_use]
    pub const fn recipient(&self) -> Option<&DeliveryRecipient> {
        self.recipient.as_ref()
    }

    #[must_use]
    pub const fn instance_id(&self) -> &CeremonyId {
        &self.instance_id
    }
}
