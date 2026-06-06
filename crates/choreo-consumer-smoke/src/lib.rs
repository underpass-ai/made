//! Generic consumer-smoke harness for the Choreographer's public
//! surface (Epic 12).
//!
//! Three chains live here. They drive the choreographer through its
//! supported APIs (`tonic` gRPC client + `async-nats` core publisher
//! and subscriber) and report a typed [`ChainOutcome`] so callers can
//! assert against structured fields rather than parse strings.
//!
//! - [`chain1`] — trigger event → (kernel bundle, simulated) →
//!   `RunCouncilDecision` reevaluation → validated remedy proposal
//!   OR escalation decision (in `Warn` mode, an unsatisfied contract
//!   still surfaces a top-ranked candidate with `passed = false` —
//!   that IS the escalation signal a consumer would consume).
//!
//! - [`chain2`] — register the canonical Report `OutputContract`
//!   (the JSON Schema lives at
//!   `api/examples/output-contracts/report.schema.json`),
//!   run a Strict deliberation, assert the rejection path against
//!   the compose stack's `NoopAgent`.
//!
//! - [`positive`] — opt-in provider-backed positive path. Register an
//!   `openai` or `vllm` agent against an OpenAI-compatible endpoint,
//!   create a council, run `RunCouncilDecision` in Strict mode, and
//!   validate that the winner satisfies the canonical Report schema.
//!
//! The crate is also a thin binary (`bin/choreo-consumer-smoke`)
//! that a consumer can run against a live cluster.

pub mod bundle;
pub mod chain1;
pub mod chain2;
pub mod harness;
pub mod outcome;
pub mod positive;

pub use chain1::run_chain_1;
pub use chain2::{run_chain_2, run_chain_2_with_schema};
pub use harness::{Harness, HarnessConfig};
pub use outcome::{AssertionRecord, AssertionStatus, BusEnvelopeRecord, ChainOutcome};
pub use positive::{run_positive_path, PositivePathConfig};
