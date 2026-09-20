use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::DurationMs;

use super::{AttentionKind, AttentionOverflowPolicy, LoopLimits, QueueLimit};

/// Five seconds of quiet before two restless kinds are folded into one.
const DEFAULT_COALESCE_MS: u64 = 5_000;
const MAX_COALESCE_MS: u64 = 60_000;

/// What an integrator asked to be told about, and how insistently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionPolicy {
    kinds: BTreeSet<AttentionKind>,
    coalesce_window: DurationMs,
    max_queued: QueueLimit,
    overflow: AttentionOverflowPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inactivity_after: Option<DurationMs>,
    limits: LoopLimits,
}

impl AttentionPolicy {
    /// Construct a validated policy.
    ///
    /// An empty selection is refused rather than silently meaning
    /// "everything": a binding that asked for nothing and is woken for
    /// everything is the kind of surprise a policy exists to prevent.
    pub fn new(
        kinds: impl IntoIterator<Item = AttentionKind>,
        coalesce_window: DurationMs,
        max_queued: QueueLimit,
        overflow: AttentionOverflowPolicy,
        inactivity_after: Option<DurationMs>,
        limits: LoopLimits,
    ) -> Result<Self, DomainError> {
        let kinds: BTreeSet<AttentionKind> = kinds.into_iter().collect();
        if kinds.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "attention_policy_kinds",
            });
        }
        if coalesce_window.get() > MAX_COALESCE_MS {
            return Err(DomainError::OutOfRange {
                field: "attention_coalesce_window",
                value: coalesce_window.get() as f64,
                min: 0.0,
                max: MAX_COALESCE_MS as f64,
            });
        }
        if inactivity_after.is_some_and(|after| after.get() == 0) {
            return Err(DomainError::MustBeNonZero {
                field: "attention_inactivity_after",
            });
        }
        Ok(Self {
            kinds,
            coalesce_window,
            max_queued,
            overflow,
            inactivity_after,
            limits,
        })
    }

    /// Whether this policy asked to hear about that kind.
    #[must_use]
    pub fn admits(&self, kind: AttentionKind) -> bool {
        self.kinds.contains(&kind)
    }

    #[must_use]
    pub const fn kinds(&self) -> &BTreeSet<AttentionKind> {
        &self.kinds
    }

    /// How long two restless events of one kind are folded into one.
    #[must_use]
    pub const fn coalesce_window(&self) -> DurationMs {
        self.coalesce_window
    }

    /// Whether this kind is one the window applies to at all.
    ///
    /// Coalescing a result or a decision would lose one; only the kinds
    /// that repeat while nothing changes are folded.
    #[must_use]
    pub const fn coalesces(kind: AttentionKind) -> bool {
        matches!(
            kind,
            AttentionKind::InactivityDetected | AttentionKind::Blocked
        )
    }

    #[must_use]
    pub const fn max_queued(&self) -> QueueLimit {
        self.max_queued
    }

    #[must_use]
    pub const fn overflow(&self) -> AttentionOverflowPolicy {
        self.overflow
    }

    /// How long nothing may happen before the quiet is itself reported.
    #[must_use]
    pub const fn inactivity_after(&self) -> Option<DurationMs> {
        self.inactivity_after
    }

    #[must_use]
    pub const fn limits(&self) -> LoopLimits {
        self.limits
    }
}

impl Default for AttentionPolicy {
    fn default() -> Self {
        Self {
            kinds: AttentionKind::ALL.into_iter().collect(),
            coalesce_window: DurationMs::from_millis(DEFAULT_COALESCE_MS),
            max_queued: QueueLimit::default(),
            overflow: AttentionOverflowPolicy::default(),
            inactivity_after: None,
            limits: LoopLimits::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_hears_everything_and_folds_only_the_restless_kinds() {
        let policy = AttentionPolicy::default();
        for kind in AttentionKind::ALL {
            assert!(policy.admits(kind), "{kind}");
        }
        assert_eq!(policy.coalesce_window().get(), 5_000);
        assert_eq!(policy.max_queued().value(), 200);
        assert!(AttentionPolicy::coalesces(AttentionKind::Blocked));
        assert!(!AttentionPolicy::coalesces(AttentionKind::ResultAvailable));
    }

    #[test]
    fn a_policy_that_asked_for_nothing_is_refused() {
        let refused = AttentionPolicy::new(
            [],
            DurationMs::ZERO,
            QueueLimit::default(),
            AttentionOverflowPolicy::default(),
            None,
            LoopLimits::default(),
        );
        assert!(refused.is_err());
    }
}
