use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::{
    DeliveryAttempt, DeliveryExpiryCause, DeliveryFailureReason, HostActivationReceipt,
    HostDeliveryId, HostDeliveryLease, HostDeliveryObservation, HostDeliveryStateKind,
    ProcessedActionRef,
};

/// Where one delivery has got to, and what it carries there.
///
/// Every state after the first names the thing that put it there — a
/// lease, a receipt, an observation, a cause — so that reading the
/// ledger answers "why" without going anywhere else. The transport
/// facts live here and never in the sealed stream, which records what
/// the ceremony decided rather than how the news travelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum HostDeliveryState {
    /// Waiting for a host to be handed it.
    Queued,
    /// Handed out exclusively until the lease expires.
    Leased { lease: HostDeliveryLease },
    /// The activation adapter reached the host.
    DeliveredToHost { receipt: HostActivationReceipt },
    /// The host said what it saw.
    Acknowledged {
        observation: HostDeliveryObservation,
    },
    /// The host acted, and said which act closed it.
    Processed {
        #[serde(with = "time::serde::rfc3339")]
        at: OffsetDateTime,
        action: ProcessedActionRef,
    },
    /// Attempts ran out, and the failure stays visible.
    Failed {
        reason: DeliveryFailureReason,
        attempt: DeliveryAttempt,
    },
    /// It stopped being worth making.
    Expired {
        #[serde(with = "time::serde::rfc3339")]
        at: OffsetDateTime,
        cause: DeliveryExpiryCause,
    },
    /// Its destination was replaced.
    ///
    /// `by` names the delivery opened in its place, and is absent when
    /// the work was told not to follow: a superseded delivery that
    /// pointed at a record nobody opened would send an operator looking
    /// for a hand-off that never happened.
    Superseded {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<HostDeliveryId>,
    },
}

impl HostDeliveryState {
    #[must_use]
    pub const fn kind(&self) -> HostDeliveryStateKind {
        match self {
            Self::Queued => HostDeliveryStateKind::Queued,
            Self::Leased { .. } => HostDeliveryStateKind::Leased,
            Self::DeliveredToHost { .. } => HostDeliveryStateKind::DeliveredToHost,
            Self::Acknowledged { .. } => HostDeliveryStateKind::Acknowledged,
            Self::Processed { .. } => HostDeliveryStateKind::Processed,
            Self::Failed { .. } => HostDeliveryStateKind::Failed,
            Self::Expired { .. } => HostDeliveryStateKind::Expired,
            Self::Superseded { .. } => HostDeliveryStateKind::Superseded,
        }
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.kind().is_terminal()
    }

    /// The lease this state holds, when it holds one.
    #[must_use]
    pub const fn lease(&self) -> Option<&HostDeliveryLease> {
        match self {
            Self::Leased { lease } => Some(lease),
            _ => None,
        }
    }

    /// What the host said, when it has said anything.
    #[must_use]
    pub const fn observation(&self) -> Option<&HostDeliveryObservation> {
        match self {
            Self::Acknowledged { observation } => Some(observation),
            _ => None,
        }
    }

    /// What replaced this delivery, when anything did.
    #[must_use]
    pub const fn superseded_by(&self) -> Option<&HostDeliveryId> {
        match self {
            Self::Superseded { by } => by.as_ref(),
            _ => None,
        }
    }

    /// The act that closed this delivery, when one has.
    #[must_use]
    pub const fn action(&self) -> Option<&ProcessedActionRef> {
        match self {
            Self::Processed { action, .. } => Some(action),
            _ => None,
        }
    }

    /// Why the last attempt did not work, when one did not.
    #[must_use]
    pub const fn failure_reason(&self) -> Option<&DeliveryFailureReason> {
        match self {
            Self::Failed { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queued_delivery_carries_nothing_and_is_not_terminal() {
        let state = HostDeliveryState::Queued;
        assert_eq!(state.kind(), HostDeliveryStateKind::Queued);
        assert!(!state.is_terminal());
        assert!(state.lease().is_none());
        assert!(state.observation().is_none());
        assert!(state.action().is_none());
        assert!(state.failure_reason().is_none());
    }
}
