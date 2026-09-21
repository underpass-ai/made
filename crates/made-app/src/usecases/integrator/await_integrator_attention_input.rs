//! What a host says when it comes to ask for its work.

use made_core::ports::HostDeliveryPageLimit;
use made_core::value_objects::{
    DurationMs, HostAgentIncarnation, IntegratorBindingId, IntegratorFence, IntegratorScope,
};

/// The maximum a host may hold the line for, whatever it asks.
pub const MAX_WAIT: DurationMs = DurationMs::from_millis(30_000);

/// The request to be handed whatever this integrator is owed.
///
/// The incarnation and the fence are the host saying which process it
/// is and which generation of the binding it belongs to. Both are
/// checked before anything is leased, because a host that was replaced
/// asking for work is not an error to log — it is the one thing the
/// fence exists to refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwaitIntegratorAttentionInput {
    pub scope: IntegratorScope,
    pub binding_id: IntegratorBindingId,
    pub incarnation: HostAgentIncarnation,
    pub fence: IntegratorFence,
    pub limit: HostDeliveryPageLimit,
    /// How long to hold the line when there is nothing yet. Capped at
    /// [`MAX_WAIT`], like the ceremony stream: a bounded wait is a
    /// wait a caller can retry, and an unbounded one is a hung host.
    pub wait: DurationMs,
    pub lease_duration: DurationMs,
}

impl AwaitIntegratorAttentionInput {
    /// The wait this request actually gets.
    #[must_use]
    pub fn bounded_wait(&self) -> DurationMs {
        if self.wait.get() > MAX_WAIT.get() {
            MAX_WAIT
        } else {
            self.wait
        }
    }
}
