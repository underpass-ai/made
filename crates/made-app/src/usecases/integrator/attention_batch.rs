//! What a host gets back when it comes to ask.

use crate::services::attention::LoopState;

use super::{AttentionDelivery, AttentionEndReason};

/// One answer to one ask: what there is to do, and whether to come
/// back.
///
/// The loop state is in every batch, including the empty ones, because
/// an empty batch has two very different meanings — nothing has
/// happened yet, or nothing is ever going to — and a host that could
/// not tell them apart would either give up early or poll a finished
/// ceremony forever.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionBatch {
    items: Vec<AttentionDelivery>,
    loop_state: LoopState,
    end_reason: AttentionEndReason,
}

impl AttentionBatch {
    #[must_use]
    pub const fn new(
        items: Vec<AttentionDelivery>,
        loop_state: LoopState,
        end_reason: AttentionEndReason,
    ) -> Self {
        Self {
            items,
            loop_state,
            end_reason,
        }
    }

    #[must_use]
    pub fn items(&self) -> &[AttentionDelivery] {
        &self.items
    }

    #[must_use]
    pub const fn loop_state(&self) -> LoopState {
        self.loop_state
    }

    #[must_use]
    pub const fn end_reason(&self) -> AttentionEndReason {
        self.end_reason
    }

    /// Whether the host should stop and talk to a person.
    #[must_use]
    pub const fn is_halting(&self) -> bool {
        self.loop_state.is_halting()
    }
}
