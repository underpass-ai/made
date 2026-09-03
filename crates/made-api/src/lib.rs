//! The published contract of the embedded MADE.
//!
//! This crate is what a consuming product is allowed to know. It holds plain
//! views, a capability report, an error vocabulary and one trait — and no
//! domain types, no adapters, no storage. A consumer that compiles against this
//! crate alone can be tested with a stub and swapped onto any implementation
//! that honours the same contract.
//!
//! Versioned deliberately. [`CONTRACT_VERSION`] moves when the meaning of this
//! surface changes, independently of the library's own release number: two
//! builds of the same release can differ in features, and a consumer that
//! guessed capabilities from a version string would find out mid-run. Consumers
//! check [`ApiCapabilities`] at startup instead.
//!
//! Vocabulary note (ADR-001): these types speak the engine's own language —
//! ceremonies. A consuming product maps them to its own terms at its own
//! boundary; nothing of that product's vocabulary appears here.

mod api_capabilities;
mod api_error;
mod authoring_views;
mod ceremony_engine_api;
mod ceremony_participant;
mod ceremony_summary;
mod definition_defect_view;
mod intervention_response_view;
mod intervention_views;
mod published_definition_view;
mod raise_intervention_request;
mod respond_to_intervention_request;
mod start_ceremony_request;

pub use api_capabilities::ApiCapabilities;
pub use api_error::ApiError;
pub use authoring_views::DefinitionAnalysisView;
pub use ceremony_engine_api::CeremonyEngineApi;
pub use ceremony_participant::CeremonyParticipant;
pub use ceremony_summary::CeremonySummary;
pub use definition_defect_view::DefinitionDefectView;
pub use intervention_response_view::InterventionResponseView;
pub use intervention_views::InterventionView;
pub use published_definition_view::PublishedDefinitionView;
pub use raise_intervention_request::RaiseInterventionRequest;
pub use respond_to_intervention_request::RespondToInterventionRequest;
pub use start_ceremony_request::StartCeremonyRequest;

/// The revision of this contract.
///
/// Moves on meaning, not on release: adding a capability keeps the version,
/// changing what an existing field or method means raises it.
pub const CONTRACT_VERSION: u32 = 3;
