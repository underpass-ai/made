//! The integrator loop's reading of the global feed.
//!
//! A ceremony decides things; an integrator has to be told about the
//! ones it can act on. Everything in this module derives that telling
//! from records that already exist — nothing here appends to a
//! journal, and two integrators reading the same ceremony reach the
//! same conclusions without coordinating.
//!
//! The reading is pure and lives in the rules. Everything that turns a
//! reading into work a host is handed — the durable cursor, the
//! delivery ledger, waking the host, keeping the queue bounded — is
//! the projector's, and none of it can change what the rules decided.

mod attention_audience;
mod attention_audience_resolver;
mod attention_backpressure;
mod attention_event;
mod attention_projector;
mod attention_recovery;
mod attention_replay;
mod attention_rules;
mod attention_subscriber;
mod binding_deliveries;
mod ceremony_definition_lookup;
mod event_ref;
mod human_decision_rule;
mod loop_progress;
mod loop_round_tally;
mod loop_stall;
mod loop_state;
mod no_progress_detector;
mod projection_pass;
mod projection_round;
mod result_acceptance;
mod session_definition_lookup;

pub use attention_audience::AttentionAudience;
pub use attention_audience_resolver::AttentionAudienceResolver;
pub use attention_event::AttentionEvent;
pub use attention_projector::AttentionProjector;
pub use attention_recovery::AttentionRecovery;
pub use attention_replay::replay;
pub use attention_rules::{attention_for, queue_overflow};
pub use attention_subscriber::AttentionSubscriber;
pub use binding_deliveries::BindingDeliveries;
pub use ceremony_definition_lookup::CeremonyDefinitionLookup;
pub use event_ref::EventRef;
pub use human_decision_rule::{human_decisions_requested, moves_the_session};
pub use loop_progress::LoopProgress;
pub use loop_round_tally::LoopRoundTally;
pub use loop_stall::LoopStall;
pub use loop_state::LoopState;
pub use no_progress_detector::NoProgressDetector;
pub use projection_round::ProjectionRound;

use projection_pass::ProjectionPass;
pub use result_acceptance::ResultAcceptance;
pub use session_definition_lookup::SessionDefinitionLookup;

use attention_backpressure::AttentionBackpressure;
