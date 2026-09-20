use serde::{Deserialize, Serialize};

/// What a successor does about the ledger its predecessor was spending
/// against.
///
/// Only [`Self::Fresh`] is honoured in this release.
/// [`Self::TransferRemaining`] is spelled here rather than left out so
/// a caller who wants it is refused with a reason instead of silently
/// given something else: reservations, root accounting and refunds are
/// held per ceremony, and moving a balance without the reservation
/// ledger following it would leave two ceremonies believing they hold
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetDisposition {
    Fresh,
    TransferRemaining,
}

impl BudgetDisposition {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::TransferRemaining => "transfer_remaining",
        }
    }
}
