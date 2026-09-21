//! In-process distribution of the MADE ceremony engine.
//!
//! [`EmbeddedMade`] executes the same `made-app` use cases as the
//! deployable service without opening sockets or reading process-wide
//! configuration. Hosts may use the local defaults or inject any adapter that
//! implements the ports from `made-core`.

#![deny(missing_debug_implementations)]

mod agentic_system_ports;
mod callback_ceremony_evidence_source;
mod callback_ceremony_step_handler;
mod embedded_authorization_scope_resolver;
mod embedded_authorization_services;
mod embedded_authorization_wiring;
mod embedded_ceremony_projection;
mod embedded_ceremony_search;
mod embedded_council_services;
mod embedded_made;
mod embedded_made_builder;
mod embedded_store_openers;
mod engine_api;
mod host_delivery_ports;
mod in_process_ceremony_definition_source;
mod integrator_loop_ports;
mod unconfigured_executor;

pub use agentic_system_ports::AgenticSystemPorts;
pub use callback_ceremony_evidence_source::CallbackCeremonyEvidenceSource;
pub use callback_ceremony_step_handler::CallbackCeremonyStepHandler;
pub use embedded_ceremony_projection::EmbeddedCeremonyProjection;
pub use embedded_made::{
    EmbeddedCeremonyAuthority, EmbeddedCeremonyOperationAuthority, EmbeddedCeremonyProjectionData,
    EmbeddedMade,
};
pub use embedded_made_builder::EmbeddedMadeBuilder;
pub use host_delivery_ports::HostDeliveryPorts;
pub use in_process_ceremony_definition_source::InProcessCeremonyDefinitionSource;
pub use integrator_loop_ports::IntegratorLoopPorts;

/// MADE release version used by this embedded distribution.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
