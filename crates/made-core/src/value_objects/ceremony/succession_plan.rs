use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    BudgetDisposition, CarriedEvidence, CeremonyId, ClaimDisposition, DefinitionPin,
    IdempotencyKey, StepId,
};
use crate::value_objects::AuditActorId;

/// The whole of a handoff, sealed in the predecessor before anything
/// is opened.
///
/// It carries plan identity, the successor's derived id, the
/// definition the successor runs, what the successor starts with, what
/// happens to every outstanding claim and what the successor does
/// about the ledger. One fact rather than several, because a partial
/// handoff — a successor named with no dispositions, dispositions with
/// no successor — is not a state this engine should be able to be in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuccessionPlan {
    plan_id: IdempotencyKey,
    successor_id: CeremonyId,
    successor_definition: DefinitionPin,
    carried: Vec<CarriedEvidence>,
    dispositions: Vec<ClaimDisposition>,
    budget: BudgetDisposition,
    planned_by: AuditActorId,
    #[serde(with = "time::serde::rfc3339")]
    planned_at: OffsetDateTime,
}

impl SuccessionPlan {
    #[must_use]
    pub const fn new(
        plan_id: IdempotencyKey,
        successor_id: CeremonyId,
        successor_definition: DefinitionPin,
        carried: Vec<CarriedEvidence>,
        dispositions: Vec<ClaimDisposition>,
        budget: BudgetDisposition,
        planned_by: AuditActorId,
        planned_at: OffsetDateTime,
    ) -> Self {
        Self {
            plan_id,
            successor_id,
            successor_definition,
            carried,
            dispositions,
            budget,
            planned_by,
            planned_at,
        }
    }

    #[must_use]
    pub const fn plan_id(&self) -> &IdempotencyKey {
        &self.plan_id
    }

    #[must_use]
    pub const fn successor_id(&self) -> &CeremonyId {
        &self.successor_id
    }

    #[must_use]
    pub const fn successor_definition(&self) -> &DefinitionPin {
        &self.successor_definition
    }

    #[must_use]
    pub fn carried(&self) -> &[CarriedEvidence] {
        &self.carried
    }

    #[must_use]
    pub fn dispositions(&self) -> &[ClaimDisposition] {
        &self.dispositions
    }

    #[must_use]
    pub const fn budget(&self) -> BudgetDisposition {
        self.budget
    }

    #[must_use]
    pub const fn planned_by(&self) -> &AuditActorId {
        &self.planned_by
    }

    #[must_use]
    pub const fn planned_at(&self) -> OffsetDateTime {
        self.planned_at
    }

    /// The disposition this plan holds for one step, if it holds one.
    #[must_use]
    pub fn disposition_for(&self, step_id: &StepId) -> Option<&ClaimDisposition> {
        self.dispositions
            .iter()
            .find(|disposition| disposition.step_id() == step_id)
    }
}
