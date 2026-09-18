use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{CeremonyEndReason, CeremonyLifecyclePhase, LifecycleReason};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyLifecycle {
    phase: CeremonyLifecyclePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_reason: Option<CeremonyEndReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<LifecycleReason>,
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    changed_at: Option<OffsetDateTime>,
}

impl Default for CeremonyLifecycle {
    fn default() -> Self {
        Self {
            phase: CeremonyLifecyclePhase::Running,
            end_reason: None,
            reason: None,
            changed_at: None,
        }
    }
}

impl CeremonyLifecycle {
    #[must_use]
    pub fn completed(at: OffsetDateTime) -> Self {
        Self {
            phase: CeremonyLifecyclePhase::Ended,
            end_reason: Some(CeremonyEndReason::Completed),
            reason: None,
            changed_at: Some(at),
        }
    }

    #[must_use]
    pub fn phase(&self) -> CeremonyLifecyclePhase {
        self.phase
    }
    #[must_use]
    pub fn end_reason(&self) -> Option<CeremonyEndReason> {
        self.end_reason
    }
    #[must_use]
    pub fn reason(&self) -> Option<&LifecycleReason> {
        self.reason.as_ref()
    }
    #[must_use]
    pub fn changed_at(&self) -> Option<OffsetDateTime> {
        self.changed_at
    }
    #[must_use]
    pub fn admits_new_work(&self) -> bool {
        self.phase == CeremonyLifecyclePhase::Running
    }
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.phase == CeremonyLifecyclePhase::Paused
    }
    #[must_use]
    pub fn is_ended(&self) -> bool {
        self.phase == CeremonyLifecyclePhase::Ended
    }
    #[must_use]
    pub fn is_explicit(&self) -> bool {
        self.changed_at.is_some()
    }
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
    pub(crate) fn pause(&mut self, reason: LifecycleReason, at: OffsetDateTime) {
        self.phase = CeremonyLifecyclePhase::Paused;
        self.reason = Some(reason);
        self.changed_at = Some(at);
    }
    pub(crate) fn resume(&mut self, at: OffsetDateTime) {
        self.phase = CeremonyLifecyclePhase::Running;
        self.reason = None;
        self.changed_at = Some(at);
    }
    pub(crate) fn end(
        &mut self,
        end_reason: CeremonyEndReason,
        reason: Option<LifecycleReason>,
        at: OffsetDateTime,
    ) {
        self.phase = CeremonyLifecyclePhase::Ended;
        self.end_reason = Some(end_reason);
        self.reason = reason;
        self.changed_at = Some(at);
    }
}
