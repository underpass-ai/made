//! The integrator loop's reading of the global feed.
//!
//! A ceremony decides things; an integrator has to be told about the
//! ones it can act on. Everything in this module derives that telling
//! from records that already exist — nothing here appends to a
//! journal, and two integrators reading the same ceremony reach the
//! same conclusions without coordinating.

mod attention_event;
mod attention_rules;
mod event_ref;
mod loop_progress;
mod loop_state;
mod result_acceptance;

pub use attention_event::AttentionEvent;
pub use attention_rules::attention_for;
pub use event_ref::EventRef;
pub use loop_progress::LoopProgress;
pub use loop_state::LoopState;
pub use result_acceptance::ResultAcceptance;
