//! Payloads of [`ProtocolEvent`](super::ProtocolEvent), one per
//! variant.
//!
//! Each payload holds everything the aggregate needs to redo the
//! mutation it records without consulting the definition or the clock,
//! built from the value objects the aggregate already keeps. Every
//! payload here is at schema version 1.

mod child_completion_accepted;
mod child_spawn_plan_adopted;
mod child_spawn_planned;
mod context_written;
mod evidence_collected;
mod execution_receipt_linked;
mod human_approval_recorded;
mod human_deferral_recorded;
mod instance_imported;
mod intervention_closed;
mod intervention_delivery_acknowledged;
mod intervention_requested;
mod intervention_responded;
mod late_step_result_observed;
mod memory_recalled;
mod participants_bound;
mod protocol_cancelled;
mod protocol_completed;
mod protocol_deadline_exceeded;
mod protocol_instance_started;
mod protocol_paused;
mod protocol_resumed;
mod reason_asserted;
mod state_deadline_exceeded;
mod state_iteration_started;
mod state_visit_entry;
mod step_completed;
mod step_deadline_exceeded;
mod step_failed;
mod step_started;
mod succession_carried;
mod successor_planned;
mod transition_applied;

pub use child_completion_accepted::ChildCompletionAccepted;
pub use child_spawn_plan_adopted::ChildSpawnPlanAdopted;
pub use child_spawn_planned::ChildSpawnPlanned;
pub use context_written::ContextWritten;
pub use evidence_collected::EvidenceCollected;
pub use execution_receipt_linked::ExecutionReceiptLinked;
pub use human_approval_recorded::HumanApprovalRecorded;
pub use human_deferral_recorded::HumanDeferralRecorded;
pub use instance_imported::InstanceImported;
pub use intervention_closed::InterventionClosed;
pub use intervention_delivery_acknowledged::InterventionDeliveryAcknowledged;
pub use intervention_requested::InterventionRequested;
pub use intervention_responded::InterventionResponded;
pub use late_step_result_observed::LateStepResultObserved;
pub use memory_recalled::MemoryRecalled;
pub use participants_bound::ParticipantsBound;
pub use protocol_cancelled::ProtocolCancelled;
pub use protocol_completed::ProtocolCompleted;
pub use protocol_deadline_exceeded::ProtocolDeadlineExceeded;
pub use protocol_instance_started::ProtocolInstanceStarted;
pub use protocol_paused::ProtocolPaused;
pub use protocol_resumed::ProtocolResumed;
pub use reason_asserted::ReasonAsserted;
pub use state_deadline_exceeded::StateDeadlineExceeded;
pub use state_iteration_started::StateIterationStarted;
pub use state_visit_entry::StateVisitEntry;
pub use step_completed::StepCompleted;
pub use step_deadline_exceeded::StepDeadlineExceeded;
pub use step_failed::StepFailed;
pub use step_started::StepStarted;
pub use succession_carried::SuccessionCarried;
pub use successor_planned::SuccessorPlanned;
pub use transition_applied::TransitionApplied;

mod step_lease_renewed;
pub use step_lease_renewed::StepLeaseRenewed;
mod host_handoff_recorded;
pub use host_handoff_recorded::HostHandoffRecorded;
