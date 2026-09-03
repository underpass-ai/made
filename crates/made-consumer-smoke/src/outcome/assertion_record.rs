use std::time::Duration;

use super::AssertionStatus;

#[derive(Debug, Clone)]
pub struct AssertionRecord {
    pub name: &'static str,
    pub status: AssertionStatus,
    pub duration: Duration,
}

impl AssertionRecord {
    #[must_use]
    pub fn passed(name: &'static str, duration: Duration) -> Self {
        Self {
            name,
            status: AssertionStatus::Passed,
            duration,
        }
    }

    #[must_use]
    pub fn skipped(name: &'static str, reason: impl Into<String>) -> Self {
        Self {
            name,
            status: AssertionStatus::Skipped {
                reason: reason.into(),
            },
            duration: Duration::ZERO,
        }
    }

    #[must_use]
    pub fn failed(name: &'static str, detail: impl Into<String>, duration: Duration) -> Self {
        Self {
            name,
            status: AssertionStatus::Failed {
                detail: detail.into(),
            },
            duration,
        }
    }

    #[must_use]
    pub fn is_passed(&self) -> bool {
        matches!(self.status, AssertionStatus::Passed)
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        matches!(self.status, AssertionStatus::Failed { .. })
    }

    #[must_use]
    pub fn is_skipped(&self) -> bool {
        matches!(self.status, AssertionStatus::Skipped { .. })
    }
}
