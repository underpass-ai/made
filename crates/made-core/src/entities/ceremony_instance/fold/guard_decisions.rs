use crate::entities::ceremony_events::{HumanApprovalRecorded, HumanDeferralRecorded};
use crate::entities::CeremonyInstance;

impl CeremonyInstance {
    /// The context is what a guard is evaluated against, so the
    /// approval is written there as well as kept as the record of
    /// who gave it. Marking the context was checked when the
    /// approval was decided; the only way it can fail here is an
    /// attribute limit the same context already passed.
    pub(super) fn apply_human_approval_recorded(&mut self, recorded: &HumanApprovalRecorded) {
        if let Ok(context) = self
            .context
            .clone()
            .with_guard_approval(recorded.approval.guard_name())
        {
            self.context = context;
        }
        self.guard_approvals.push(recorded.approval.clone());
        self.updated_at = recorded.approval.approved_at();
    }

    pub(super) fn apply_human_deferral_recorded(&mut self, recorded: &HumanDeferralRecorded) {
        self.guard_deferrals.push(recorded.deferral.clone());
        self.updated_at = recorded.deferral.deferred_at();
    }
}
