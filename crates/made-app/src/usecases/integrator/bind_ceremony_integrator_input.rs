//! What it takes to put an integrator in charge of a scope.

use made_core::ports::BindReplacement;
use made_core::value_objects::{
    FollowReplacement, HostAgentIncarnation, HostDestination, IntegratorBindingId, IntegratorScope,
    RoleId,
};

/// The request to bind one host to one ceremony or one system run.
///
/// The incarnation is the caller's, not the engine's: a host that
/// restarts is a different incarnation of the same seat, and saying so
/// is what lets the fence tell the new process from the old one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindCeremonyIntegratorInput {
    pub binding_id: IntegratorBindingId,
    pub scope: IntegratorScope,
    pub role_id: RoleId,
    pub destination: HostDestination,
    pub incarnation: HostAgentIncarnation,
    /// Whether a live binding for this scope may be displaced.
    pub replacement: BindReplacement,
    /// Whether the outgoing host's unanswered work follows to the new
    /// destination, or stays behind with the reason it stopped.
    pub follow: FollowReplacement,
}
