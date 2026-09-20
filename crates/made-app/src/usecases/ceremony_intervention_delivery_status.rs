use made_core::value_objects::{DeliveryExpiryCause, DeliveryFailureReason};

/// How far an intervention has got towards the agent it was put to.
///
/// Computed, never stored, and computed from two sources that cannot
/// flatter each other: the sealed stream says what was asked, answered
/// and closed, and the delivery ledger says what was offered, leased,
/// pushed and acknowledged. Neither on its own can say `Delivered`,
/// which is the point — a host-reported label claiming an intervention
/// was delivered is a claim by the reporter, and this is not built out
/// of claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyInterventionDeliveryStatus {
    /// Sealed in the stream; nothing has been offered to a host yet.
    Recorded,
    /// Waiting in the ledger for a host to take it.
    Queued,
    /// In a host's hands: leased to it, or pushed to it with a receipt.
    ///
    /// Not "the host read it". The engine knows it handed the item
    /// over and no more than that, which is why `Acknowledged` is a
    /// separate state and why only the host can move it there.
    Delivered,
    /// A named agent said it saw the item.
    Acknowledged,
    /// The item was answered.
    Responded,
    /// The requester closed the item.
    Closed,
    /// Every attempt was used up, and the reason is kept.
    Failed(DeliveryFailureReason),
    /// It stopped being worth making, and why.
    Expired(DeliveryExpiryCause),
    /// Nothing can be said: no route was ever opened for this item.
    Unsupported,
}

impl CeremonyInterventionDeliveryStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::Queued => "queued",
            Self::Delivered => "delivered",
            Self::Acknowledged => "acknowledged",
            Self::Responded => "responded",
            Self::Closed => "closed",
            Self::Failed(_) => "failed",
            Self::Expired(_) => "expired",
            Self::Unsupported => "unsupported",
        }
    }

    /// Why this item stopped, in the words the ledger recorded.
    #[must_use]
    pub fn reason(&self) -> Option<String> {
        match self {
            Self::Failed(reason) => Some(reason.to_string()),
            Self::Expired(cause) => Some(cause.to_string()),
            _ => None,
        }
    }

    /// Whether somebody still owes this item something.
    #[must_use]
    pub const fn is_unresolved(&self) -> bool {
        matches!(
            self,
            Self::Recorded | Self::Queued | Self::Delivered | Self::Acknowledged
        )
    }

    /// Whether the engine has evidence a host holds the item.
    ///
    /// An acknowledgement is evidence; a lease or an activation
    /// receipt is evidence of an offer. `Recorded` and `Queued` are
    /// neither, and no other state may be reported as delivery.
    #[must_use]
    pub const fn has_reached_a_host(&self) -> bool {
        matches!(
            self,
            Self::Delivered | Self::Acknowledged | Self::Responded | Self::Closed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_before_a_real_offer_counts_as_having_reached_a_host() {
        assert!(!CeremonyInterventionDeliveryStatus::Recorded.has_reached_a_host());
        assert!(!CeremonyInterventionDeliveryStatus::Queued.has_reached_a_host());
        assert!(!CeremonyInterventionDeliveryStatus::Unsupported.has_reached_a_host());
        assert!(CeremonyInterventionDeliveryStatus::Delivered.has_reached_a_host());
        assert!(CeremonyInterventionDeliveryStatus::Acknowledged.has_reached_a_host());
    }

    #[test]
    fn an_ended_item_is_resolved_and_a_waiting_one_is_not() {
        assert!(CeremonyInterventionDeliveryStatus::Acknowledged.is_unresolved());
        assert!(!CeremonyInterventionDeliveryStatus::Responded.is_unresolved());
        assert!(!CeremonyInterventionDeliveryStatus::Closed.is_unresolved());
        assert!(
            !CeremonyInterventionDeliveryStatus::Expired(DeliveryExpiryCause::CeremonyEnded)
                .is_unresolved()
        );
    }

    #[test]
    fn a_stopped_item_keeps_the_reason_it_stopped_for() {
        let failed = CeremonyInterventionDeliveryStatus::Failed(
            DeliveryFailureReason::new("host refused the item").unwrap(),
        );
        assert_eq!(failed.as_str(), "failed");
        assert_eq!(failed.reason().unwrap(), "host refused the item");
        assert!(CeremonyInterventionDeliveryStatus::Queued
            .reason()
            .is_none());
    }
}
