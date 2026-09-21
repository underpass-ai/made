//! What a host gets back when it comes to ask.

use made_core::value_objects::GlobalPosition;

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
    journal_head: Option<GlobalPosition>,
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
            journal_head: None,
        }
    }

    /// The batch, saying how far the feed had been projected for this
    /// binding when it was built.
    ///
    /// A host that asks twice and is told the same head twice has been
    /// told nothing new, whatever else the answer carried. That is the
    /// same reading the engine stops itself on, so a host can see the
    /// stall coming instead of being surprised by it.
    #[must_use]
    pub const fn at(mut self, journal_head: Option<GlobalPosition>) -> Self {
        self.journal_head = journal_head;
        self
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

    /// How far the feed had been projected for this binding.
    #[must_use]
    pub const fn journal_head(&self) -> Option<GlobalPosition> {
        self.journal_head
    }

    /// Whether the host should stop and talk to a person.
    #[must_use]
    pub const fn is_halting(&self) -> bool {
        self.loop_state.is_halting()
    }
}
