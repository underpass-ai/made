//! Payloads of [`CeremonyEvent`](super::CeremonyEvent), one per
//! variant.
//!
//! Each payload holds everything the aggregate needs to redo the
//! mutation it records without consulting the definition or the clock,
//! built from the value objects the aggregate already keeps. Every
//! payload here is at schema version 1.

mod ceremony_completed;
mod ceremony_instance_started;
mod evidence_collected;
mod human_approval_recorded;
mod human_deferral_recorded;
mod intervention_closed;
mod intervention_requested;
mod intervention_responded;
mod memory_recalled;
mod participants_bound;
mod reason_asserted;
mod step_completed;
mod step_failed;
mod step_started;
mod transition_applied;

pub use ceremony_completed::CeremonyCompleted;
pub use ceremony_instance_started::CeremonyInstanceStarted;
pub use evidence_collected::EvidenceCollected;
pub use human_approval_recorded::HumanApprovalRecorded;
pub use human_deferral_recorded::HumanDeferralRecorded;
pub use intervention_closed::InterventionClosed;
pub use intervention_requested::InterventionRequested;
pub use intervention_responded::InterventionResponded;
pub use memory_recalled::MemoryRecalled;
pub use participants_bound::ParticipantsBound;
pub use reason_asserted::ReasonAsserted;
pub use step_completed::StepCompleted;
pub use step_failed::StepFailed;
pub use step_started::StepStarted;
pub use transition_applied::TransitionApplied;
