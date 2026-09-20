use serde::{Deserialize, Serialize};

use time::OffsetDateTime;

use super::{
    RoleId, SourceRecordRef, StateIteration, StateVisit, StepAttempt, StepErrorMessage,
    StepIteration, StepLease, StepOutput, StepResult, StepStatus,
};
use crate::value_objects::BudgetReservationId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepExecutionRecord {
    status: StepStatus,
    #[serde(default, skip_serializing_if = "StateIteration::is_first")]
    state_iteration: StateIteration,
    #[serde(default, skip_serializing_if = "StateVisit::is_first")]
    state_visit: StateVisit,
    #[serde(default)]
    iteration: StepIteration,
    attempt: StepAttempt,
    lease: Option<StepLease>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    renewed_expires_at: Option<OffsetDateTime>,
    output: StepOutput,
    error_message: Option<StepErrorMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    claimed_role: Option<RoleId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget_reservation_id: Option<BudgetReservationId>,
    /// Where this record's work actually happened, when it happened
    /// somewhere else.
    ///
    /// Present only on a record a successor carried from its
    /// predecessor. Absent — and skipped on the wire, so no sealed
    /// record moves a byte — for every record of work this instance
    /// did itself, which is every record written before successions
    /// existed and nearly every one written since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    carried_from: Option<SourceRecordRef>,
}

impl StepExecutionRecord {
    #[must_use]
    pub fn pending() -> Self {
        Self {
            status: StepStatus::Pending,
            state_visit: StateVisit::FIRST,
            state_iteration: StateIteration::FIRST,
            iteration: StepIteration::FIRST,
            attempt: StepAttempt::FIRST,
            lease: None,
            renewed_expires_at: None,
            output: StepOutput::empty(),
            error_message: None,
            claimed_role: None,
            budget_reservation_id: None,
            carried_from: None,
        }
    }

    #[must_use]
    pub fn pending_iteration(iteration: StepIteration) -> Self {
        Self {
            iteration,
            ..Self::pending()
        }
    }

    #[must_use]
    pub fn pending_state_iteration(state_iteration: StateIteration) -> Self {
        Self {
            state_iteration,
            ..Self::pending()
        }
    }

    #[must_use]
    pub fn pending_coordinates(state_iteration: StateIteration, iteration: StepIteration) -> Self {
        Self {
            state_iteration,
            iteration,
            ..Self::pending()
        }
    }

    #[must_use]
    pub fn state_visit(&self) -> StateVisit {
        self.state_visit
    }

    #[must_use]
    pub fn with_state_visit(mut self, state_visit: StateVisit) -> Self {
        self.state_visit = state_visit;
        self
    }

    #[must_use]
    pub fn state_iteration(&self) -> StateIteration {
        self.state_iteration
    }

    #[must_use]
    pub fn status(&self) -> StepStatus {
        self.status
    }

    #[must_use]
    pub fn iteration(&self) -> StepIteration {
        self.iteration
    }

    #[must_use]
    pub fn attempt(&self) -> StepAttempt {
        self.attempt
    }

    #[must_use]
    pub fn lease(&self) -> Option<&StepLease> {
        self.lease.as_ref()
    }

    #[must_use]
    pub fn effective_lease_expires_at(&self) -> Option<OffsetDateTime> {
        self.renewed_expires_at
            .or_else(|| self.lease.as_ref().map(StepLease::expires_at))
    }

    pub(crate) fn renew_lease_until(&mut self, expires_at: OffsetDateTime) {
        self.renewed_expires_at = Some(expires_at);
    }

    #[must_use]
    pub fn output(&self) -> &StepOutput {
        &self.output
    }

    #[must_use]
    pub fn error_message(&self) -> Option<&StepErrorMessage> {
        self.error_message.as_ref()
    }

    #[must_use]
    pub fn claimed_role(&self) -> Option<&RoleId> {
        self.claimed_role.as_ref()
    }

    #[must_use]
    pub fn budget_reservation_id(&self) -> Option<&BudgetReservationId> {
        self.budget_reservation_id.as_ref()
    }

    #[must_use]
    pub fn can_be_started_at(&self, now: OffsetDateTime) -> bool {
        if self.status.is_executable() {
            return true;
        }
        self.status == StepStatus::InProgress
            && self
                .effective_lease_expires_at()
                .is_some_and(|expiry| now >= expiry)
    }

    #[must_use]
    pub fn has_live_lease_at(&self, now: OffsetDateTime) -> bool {
        self.status == StepStatus::InProgress
            && self
                .effective_lease_expires_at()
                .is_some_and(|expiry| now < expiry)
    }

    #[must_use]
    pub fn with_started(
        self,
        lease: StepLease,
        attempt: StepAttempt,
        claimed_role: Option<RoleId>,
    ) -> Self {
        Self {
            status: StepStatus::InProgress,
            state_iteration: self.state_iteration,
            state_visit: self.state_visit,
            iteration: self.iteration,
            attempt,
            lease: Some(lease),
            renewed_expires_at: None,
            output: self.output,
            error_message: None,
            claimed_role,
            budget_reservation_id: self.budget_reservation_id,
            carried_from: self.carried_from,
        }
    }

    #[must_use]
    pub fn with_budget_reservation(mut self, reservation_id: Option<BudgetReservationId>) -> Self {
        self.budget_reservation_id = reservation_id;
        self
    }

    #[must_use]
    pub fn with_result(self, result: StepResult) -> Self {
        let (status, output, error_message) = result.into_parts();
        Self {
            status,
            state_iteration: self.state_iteration,
            state_visit: self.state_visit,
            iteration: self.iteration,
            attempt: self.attempt,
            lease: None,
            renewed_expires_at: None,
            output,
            error_message,
            claimed_role: self.claimed_role,
            budget_reservation_id: self.budget_reservation_id,
            carried_from: self.carried_from,
        }
    }

    /// A completed record whose work was done in another instance.
    ///
    /// Built rather than folded from a start and a result, because
    /// there was no start here: nothing was claimed, nothing was
    /// leased, and the attempt is the first this instance would make.
    /// The source is what keeps the distinction readable afterwards.
    #[must_use]
    pub fn carried(
        output: StepOutput,
        source: SourceRecordRef,
        state_iteration: StateIteration,
        state_visit: StateVisit,
    ) -> Self {
        Self {
            status: StepStatus::Completed,
            state_iteration,
            state_visit,
            output,
            carried_from: Some(source),
            ..Self::pending()
        }
    }

    /// Where this record's work happened, when it was not here.
    #[must_use]
    pub const fn carried_from(&self) -> Option<&SourceRecordRef> {
        self.carried_from.as_ref()
    }
}
