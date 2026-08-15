//! In-process distribution of the MADE ceremony engine.
//!
//! [`EmbeddedMade`] executes the same `made-app` use cases as the
//! deployable service without opening sockets or reading process-wide
//! configuration. Hosts may use the local defaults or inject any adapter that
//! implements the ports from `made-core`.

#![deny(missing_debug_implementations)]

mod callback_ceremony_evidence_source;
mod callback_ceremony_step_handler;
mod embedded_made;
mod embedded_made_builder;
mod engine_api;
mod in_process_ceremony_definition_source;

pub use callback_ceremony_evidence_source::CallbackCeremonyEvidenceSource;
pub use callback_ceremony_step_handler::CallbackCeremonyStepHandler;
pub use embedded_made::EmbeddedMade;
pub use embedded_made_builder::EmbeddedMadeBuilder;
pub use in_process_ceremony_definition_source::InProcessCeremonyDefinitionSource;

/// MADE release version used by this embedded distribution.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
