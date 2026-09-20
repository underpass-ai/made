//! Where the loop stands, so a host knows whether to act or to stop.

use made_core::value_objects::CeremonyEndReason;
use serde::Serialize;
use time::OffsetDateTime;

use crate::usecases::CeremonyInstanceView;

use super::LoopProgress;

/// What an integrator should do next, in one word.
///
/// Derived on every read from the journal and the ledger, never
/// stored. A stored loop state would be a fourth place the truth
/// lives and would drift the moment a ceremony advanced through a
/// path the loop was not watching.
///
/// Three of these are terminal for the host — `Completed`, `Failed`
/// and `Blocked` — and one, `AwaitingHumanDecision`, means the host
/// must talk to a person rather than act. Guidance tells an
/// integrator to stop on all four, which is what keeps a loop from
/// approving its own work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopState {
    /// Somebody holds a claim and is working.
    Executing,
    /// No step can be claimed and results are still owed.
    AwaitingResults,
    /// A guard needs a person, and the loop must not answer for one.
    AwaitingHumanDecision,
    /// Stuck: blocked delivery outstanding, or no progress for the
    /// allowed number of rounds.
    Blocked,
    /// The ceremony is paused; nothing is queued while it stays so.
    Paused,
    /// The ceremony reached its end.
    Completed,
    /// The ceremony was cancelled or ran out of time.
    Failed,
}

impl LoopState {
    /// Read the loop from what the ceremony says and what is owed.
    ///
    /// The order matters and is not the order of the variants. A
    /// finished ceremony is finished whatever else is outstanding; a
    /// paused one is paused even when a step could otherwise be
    /// claimed; and being stuck outranks being busy, because a host
    /// that reads `Executing` will wait rather than escalate.
    #[must_use]
    pub fn derive(
        view: &CeremonyInstanceView<'_>,
        progress: LoopProgress,
        now: OffsetDateTime,
    ) -> Self {
        let lifecycle = view.instance().lifecycle();
        if lifecycle.is_ended() {
            // An ended ceremony with no recorded reason is not a
            // success. Reading it as failure sends the host to a
            // person, which is the safe way to be wrong here.
            return if matches!(lifecycle.end_reason(), Some(CeremonyEndReason::Completed)) {
                Self::Completed
            } else {
                Self::Failed
            };
        }
        if lifecycle.is_paused() {
            return Self::Paused;
        }
        if progress == LoopProgress::Blocked {
            return Self::Blocked;
        }
        if !view.waiting_for_human().is_empty() {
            return Self::AwaitingHumanDecision;
        }
        if view
            .steps()
            .iter()
            .any(|step| step.record().has_live_lease_at(now))
        {
            return Self::Executing;
        }
        if progress == LoopProgress::AwaitingResults {
            return Self::AwaitingResults;
        }
        Self::Executing
    }

    /// Whether an integrator should stop and speak to the user.
    #[must_use]
    pub const fn is_halting(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Blocked | Self::AwaitingHumanDecision
        )
    }
}
