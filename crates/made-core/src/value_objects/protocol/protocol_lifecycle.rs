use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{LifecycleReason, ProtocolEndReason, ProtocolLifecyclePhase};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolLifecycle {
    phase: ProtocolLifecyclePhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_reason: Option<ProtocolEndReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<LifecycleReason>,
    #[serde(
        default,
        with = "time::serde::rfc3339::option",
        skip_serializing_if = "Option::is_none"
    )]
    changed_at: Option<OffsetDateTime>,
}

impl Default for ProtocolLifecycle {
    fn default() -> Self {
        Self {
            phase: ProtocolLifecyclePhase::Running,
            end_reason: None,
            reason: None,
            changed_at: None,
        }
    }
}

impl ProtocolLifecycle {
    #[must_use]
    pub fn completed(at: OffsetDateTime) -> Self {
        Self {
            phase: ProtocolLifecyclePhase::Ended,
            end_reason: Some(ProtocolEndReason::Completed),
            reason: None,
            changed_at: Some(at),
        }
    }

    #[must_use]
    pub fn phase(&self) -> ProtocolLifecyclePhase {
        self.phase
    }
    #[must_use]
    pub fn end_reason(&self) -> Option<ProtocolEndReason> {
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
        self.phase == ProtocolLifecyclePhase::Running
    }
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.phase == ProtocolLifecyclePhase::Paused
    }
    #[must_use]
    pub fn is_ended(&self) -> bool {
        self.phase == ProtocolLifecyclePhase::Ended
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
        self.phase = ProtocolLifecyclePhase::Paused;
        self.reason = Some(reason);
        self.changed_at = Some(at);
    }
    pub(crate) fn resume(&mut self, at: OffsetDateTime) {
        self.phase = ProtocolLifecyclePhase::Running;
        self.reason = None;
        self.changed_at = Some(at);
    }
    pub(crate) fn end(
        &mut self,
        end_reason: ProtocolEndReason,
        reason: Option<LifecycleReason>,
        at: OffsetDateTime,
    ) {
        self.phase = ProtocolLifecyclePhase::Ended;
        self.end_reason = Some(end_reason);
        self.reason = reason;
        self.changed_at = Some(at);
    }
}
