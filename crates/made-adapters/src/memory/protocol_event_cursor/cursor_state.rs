use made_core::value_objects::{
    GlobalPosition, ProtocolEventCursorAttempt, ProtocolEventCursorLease, QuarantinedProtocolEvent,
};

#[derive(Debug, Default)]
pub(super) struct CursorState {
    pub(super) acknowledged_through: Option<GlobalPosition>,
    pub(super) attempt: ProtocolEventCursorAttempt,
    pub(super) lease: Option<ProtocolEventCursorLease>,
    pub(super) quarantined: Vec<QuarantinedProtocolEvent>,
}
