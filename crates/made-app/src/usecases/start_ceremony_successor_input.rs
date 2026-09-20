use made_core::value_objects::{
    AuditActorId, AuditActorKind, BudgetDisposition, CeremonyContext, CeremonyId, CeremonyName,
    CeremonyVersion, ClaimDisposition, IdempotencyKey, StepId,
};

/// What it takes to hand a paused ceremony to a successor.
///
/// The plan id is the caller's, and it is what makes the whole
/// operation retryable: the successor's id derives from it, so a
/// second call with the same plan seals nothing new and opens nothing
/// twice. A second call with the same plan id and different content
/// conflicts, which is the honest answer to two different handoffs
/// wearing one name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartCeremonySuccessorInput {
    pub instance_id: CeremonyId,
    pub plan_id: IdempotencyKey,
    pub definition_name: CeremonyName,
    pub definition_version: CeremonyVersion,
    /// Steps of the successor that start from the predecessor's sealed
    /// work. Named by the caller from what planning proposed.
    pub carried: Vec<StepId>,
    pub dispositions: Vec<ClaimDisposition>,
    pub budget: BudgetDisposition,
    /// Context for the successor. Absent means the predecessor's
    /// context travels unchanged, which is the usual case: the
    /// definition changed, not what the session is about.
    pub context_overrides: Option<CeremonyContext>,
    pub actor_id: AuditActorId,
    pub actor_kind: AuditActorKind,
}
