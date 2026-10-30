use made_core::value_objects::{
    AuditActorKind, MemoryConfidence, ProtocolId, ProtocolReasonKind, ProtocolReasonRationale,
    ProtocolRecordRef, RoleId,
};

/// Saying why one thing a session produced led to another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertProtocolReasonInput {
    pub(crate) instance_id: ProtocolId,
    pub(crate) role_id: RoleId,
    /// What kind of party fills that seat.
    ///
    /// Carried, never worked out. The engine sees a seat and cannot
    /// see what fills it.
    pub(crate) role_kind: AuditActorKind,
    pub(crate) from: ProtocolRecordRef,
    pub(crate) to: ProtocolRecordRef,
    pub(crate) kind: ProtocolReasonKind,
    pub(crate) why: ProtocolReasonRationale,
    pub(crate) confidence: MemoryConfidence,
}

impl AssertProtocolReasonInput {
    #[must_use]
    pub fn new(
        instance_id: ProtocolId,
        role_id: RoleId,
        role_kind: AuditActorKind,
        from: ProtocolRecordRef,
        to: ProtocolRecordRef,
        kind: ProtocolReasonKind,
        why: impl Into<String>,
        confidence: MemoryConfidence,
    ) -> Self {
        Self {
            instance_id,
            role_id,
            role_kind,
            from,
            to,
            kind,
            why: ProtocolReasonRationale::new(why),
            confidence,
        }
    }

    #[must_use]
    pub const fn instance_id(&self) -> &ProtocolId {
        &self.instance_id
    }
}
