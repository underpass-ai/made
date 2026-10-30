use made_core::value_objects::{AuditActorKind, ProtocolId, RoleId, TransitionTrigger};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyProtocolTransitionInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) role_id: RoleId,
    /// What kind of party is firing the trigger.
    ///
    /// The seat comes from the definition, which says which seat was
    /// required. What filled it is something only the caller can see,
    /// so it is carried rather than worked out.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) trigger: TransitionTrigger,
}

impl ApplyProtocolTransitionInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        trigger: TransitionTrigger,
    ) -> Self {
        Self {
            instance_id,
            role_id,
            role_kind,
            trigger,
        }
    }

    #[must_use]
    pub fn role_kind(&self) -> AuditActorKind {
        self.role_kind
    }

    #[must_use]
    pub fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }

    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    #[must_use]
    pub fn trigger(&self) -> &TransitionTrigger {
        &self.trigger
    }
}
