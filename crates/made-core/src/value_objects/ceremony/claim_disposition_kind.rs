use serde::{Deserialize, Serialize};

use crate::value_objects::{EvidenceReference, ExecutionReceiptId};

/// What became of one outstanding claim when a successor was planned.
///
/// There is no implicit abandonment. A claim may hold a lease over
/// work that really happened outside this engine, and a handoff that
/// stayed silent about it would move the ceremony on while the effect
/// stayed behind with nobody accountable for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ClaimDispositionKind {
    /// Nothing happened outside the engine, so there is nothing to reconcile.
    AbandonNoExternalEffect,
    /// Something happened outside the engine and was settled; the
    /// evidence says where that was established.
    AbandonEffectReconciled { evidence: EvidenceReference },
    /// The work produced a receipt the successor answers for.
    CarryReceipt { receipt_id: ExecutionReceiptId },
    /// The work is to be done again, under the successor's definition.
    RetryInSuccessor,
}

impl ClaimDispositionKind {
    #[must_use]
    pub const fn as_label(&self) -> &'static str {
        match self {
            Self::AbandonNoExternalEffect => "abandon_no_external_effect",
            Self::AbandonEffectReconciled { .. } => "abandon_effect_reconciled",
            Self::CarryReceipt { .. } => "carry_receipt",
            Self::RetryInSuccessor => "retry_in_successor",
        }
    }
}
