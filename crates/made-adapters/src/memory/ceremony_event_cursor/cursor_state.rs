use made_core::value_objects::{
    CeremonyEventCursorAttempt, CeremonyEventCursorLease, GlobalPosition, QuarantinedCeremonyEvent,
};

#[derive(Debug, Default)]
pub(super) struct CursorState {
    pub(super) acknowledged_through: Option<GlobalPosition>,
    pub(super) attempt: CeremonyEventCursorAttempt,
    pub(super) lease: Option<CeremonyEventCursorLease>,
    pub(super) quarantined: Vec<QuarantinedCeremonyEvent>,
}
