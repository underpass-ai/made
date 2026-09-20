//! Delivering one item to one host destination, durably.
//!
//! A supervisor's question to a working agent and a projected reason
//! for an integrator to look at a ceremony are the same mechanism: an
//! item, a destination, an exclusive hand-out, an acknowledgement and a
//! visible end. Modelled once here so the two never drift apart.
//!
//! Nothing in this module is a ceremony fact. Queued, leased, reached,
//! expired and failed describe how news travelled, and the sealed
//! stream records what the ceremony decided — the two answer different
//! questions and are stored in different places on purpose.

mod attention_event_id;
mod attention_kind;
mod attention_overflow_policy;
mod attention_policy;
mod attention_reason;
mod attention_summary;
mod delivery_attempt;
mod delivery_attempt_limit;
mod delivery_expiry_cause;
mod delivery_failure_reason;
mod delivery_history;
mod delivery_history_entry;
mod delivery_note;
mod follow_replacement;
mod host_activation_adapter_kind;
mod host_activation_envelope;
mod host_activation_mode;
mod host_activation_receipt;
mod host_address;
mod host_delivery_id;
mod host_delivery_item;
mod host_delivery_item_kind;
mod host_delivery_lease;
mod host_delivery_lease_id;
mod host_delivery_mode;
mod host_delivery_observation;
mod host_delivery_observation_kind;
mod host_delivery_policy;
mod host_delivery_record;
mod host_delivery_state;
mod host_delivery_state_kind;
mod host_delivery_target;
mod host_delivery_target_key;
mod host_destination;
mod host_kind;
mod host_transport_ref;
mod integrator_binding;
mod integrator_binding_id;
mod integrator_fence;
mod integrator_scope;
mod integrator_scope_key;
mod loop_limits;
mod loop_round_limit;
mod processed_action_kind;
mod processed_action_ref;
mod queue_limit;

pub use attention_event_id::AttentionEventId;
pub use attention_kind::AttentionKind;
pub use attention_overflow_policy::AttentionOverflowPolicy;
pub use attention_policy::AttentionPolicy;
pub use attention_reason::AttentionReason;
pub use attention_summary::AttentionSummary;
pub use delivery_attempt::DeliveryAttempt;
pub use delivery_attempt_limit::DeliveryAttemptLimit;
pub use delivery_expiry_cause::DeliveryExpiryCause;
pub use delivery_failure_reason::DeliveryFailureReason;
pub use delivery_history::DeliveryHistory;
pub use delivery_history_entry::DeliveryHistoryEntry;
pub use delivery_note::DeliveryNote;
pub use follow_replacement::FollowReplacement;
pub use host_activation_adapter_kind::HostActivationAdapterKind;
pub use host_activation_envelope::HostActivationEnvelope;
pub use host_activation_mode::HostActivationMode;
pub use host_activation_receipt::HostActivationReceipt;
pub use host_address::HostAddress;
pub use host_delivery_id::HostDeliveryId;
pub use host_delivery_item::HostDeliveryItem;
pub use host_delivery_item_kind::HostDeliveryItemKind;
pub use host_delivery_lease::HostDeliveryLease;
pub use host_delivery_lease_id::HostDeliveryLeaseId;
pub use host_delivery_mode::HostDeliveryMode;
pub use host_delivery_observation::HostDeliveryObservation;
pub use host_delivery_observation_kind::HostDeliveryObservationKind;
pub use host_delivery_policy::HostDeliveryPolicy;
pub use host_delivery_record::HostDeliveryRecord;
pub use host_delivery_state::HostDeliveryState;
pub use host_delivery_state_kind::HostDeliveryStateKind;
pub use host_delivery_target::HostDeliveryTarget;
pub use host_delivery_target_key::HostDeliveryTargetKey;
pub use host_destination::HostDestination;
pub use host_kind::HostKind;
pub use host_transport_ref::HostTransportRef;
pub use integrator_binding::IntegratorBinding;
pub use integrator_binding_id::IntegratorBindingId;
pub use integrator_fence::IntegratorFence;
pub use integrator_scope::IntegratorScope;
pub use integrator_scope_key::IntegratorScopeKey;
pub use loop_limits::LoopLimits;
pub use loop_round_limit::LoopRoundLimit;
pub use processed_action_kind::ProcessedActionKind;
pub use processed_action_ref::ProcessedActionRef;
pub use queue_limit::QueueLimit;
