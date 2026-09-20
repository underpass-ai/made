//! Value objects for designing an agentic system.
//!
//! A ceremony coordinates one procedure. This vocabulary describes the
//! level above it: business roles, logical participants, how they
//! collaborate, which ceremonies the system composes and under what
//! supervision.
//!
//! Two rules run through the whole module. Nothing here owns a
//! ceremony — compositions reference published definitions by an
//! immutable pin — and nothing here is named for what actually ran:
//! profiles are requested, participants are logical, and what a host
//! really did belongs to the claim that recorded it.

mod agentic_system_digest;
mod agentic_system_lifecycle;
mod agentic_system_page_limit;
mod agentic_system_revision;
mod agentic_system_validation_finding;
mod agentic_system_validation_locus;
mod capability;
mod ceremony_activation;
mod ceremony_composition;
mod ceremony_execution_link;
mod ceremony_output_ref;
mod channel_name;
mod collaboration_kind;
mod collaboration_link;
mod execution_state;
mod guard_ref;
mod independence_group;
mod independence_rule;
mod link_status;
mod logical_participant;
mod loop_round;
mod loop_rounds;
mod model_name;
mod participant_binding_policy;
mod participant_id;
mod participant_kind;
mod participant_materialization;
mod reasoning_effort;
mod requested_execution_profile;
mod responsibility;
mod slug;
mod supervision_policy;
mod system_ceremony_id;
mod system_pin;
mod system_purpose;
mod system_role;
mod system_role_id;
mod system_role_kind;
mod unavailability_reason;

pub use agentic_system_digest::AgenticSystemDigest;
pub use agentic_system_lifecycle::AgenticSystemLifecycle;
pub use agentic_system_page_limit::AgenticSystemPageLimit;
pub use agentic_system_revision::AgenticSystemRevision;
pub use agentic_system_validation_finding::AgenticSystemValidationFinding;
pub use agentic_system_validation_locus::AgenticSystemValidationLocus;
pub use capability::Capability;
pub use ceremony_activation::CeremonyActivation;
pub use ceremony_composition::CeremonyComposition;
pub use ceremony_execution_link::CeremonyExecutionLink;
pub use ceremony_output_ref::CeremonyOutputRef;
pub use channel_name::ChannelName;
pub use collaboration_kind::CollaborationKind;
pub use collaboration_link::CollaborationLink;
pub use execution_state::ExecutionState;
pub use guard_ref::GuardRef;
pub use independence_group::IndependenceGroup;
pub use independence_rule::IndependenceRule;
pub use link_status::LinkStatus;
pub use logical_participant::LogicalParticipant;
pub use loop_round::LoopRound;
pub use loop_rounds::LoopRounds;
pub use model_name::ModelName;
pub use participant_binding_policy::ParticipantBindingPolicy;
pub use participant_id::ParticipantId;
pub use participant_kind::ParticipantKind;
pub use participant_materialization::ParticipantMaterialization;
pub use reasoning_effort::ReasoningEffort;
pub use requested_execution_profile::RequestedExecutionProfile;
pub use responsibility::Responsibility;
pub use supervision_policy::SupervisionPolicy;
pub use system_ceremony_id::SystemCeremonyId;
pub use system_pin::SystemPin;
pub use system_purpose::SystemPurpose;
pub use system_role::SystemRole;
pub use system_role_id::SystemRoleId;
pub use system_role_kind::SystemRoleKind;
pub use unavailability_reason::UnavailabilityReason;
