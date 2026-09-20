//! Designing, sealing and running an agentic system.
//!
//! A subfolder because the nine capabilities are one coherent surface
//! over one aggregate, and one type per file would otherwise scatter
//! them through a directory of two hundred unrelated ceremony files.
//!
//! What runs through all of it (ADR-021): the design references
//! ceremonies and never owns them; persistence is a revision log with
//! compare-and-swap rather than an event stream; a run is a separate
//! entity with its own store; and what the host could not supply is
//! recorded as unavailable rather than stood in for.

mod advance_agentic_system_execution_use_case;
mod agentic_system_ceremony_view;
mod agentic_system_composition_document;
mod agentic_system_design_document;
mod agentic_system_execution_view;
mod agentic_system_pin_document;
mod agentic_system_pins;
mod agentic_system_publication_view;
mod agentic_system_validation_view;
mod agentic_system_view;
mod ceremony_launcher;
mod design_agentic_system_use_case;
mod get_agentic_system_execution_use_case;
mod get_agentic_system_use_case;
mod instantiate_agentic_system_input;
mod instantiate_agentic_system_use_case;
mod list_agentic_systems_use_case;
mod materialization;
mod participant_offer;
mod publish_agentic_system_use_case;
mod render_agentic_system_diagram_use_case;
mod validate_agentic_system_use_case;

pub use advance_agentic_system_execution_use_case::{
    AdvanceAgenticSystemExecutionUseCase, Observation,
};
pub use agentic_system_ceremony_view::AgenticSystemCeremonyView;
pub use agentic_system_composition_document::AgenticSystemCompositionDocument;
pub use agentic_system_design_document::AgenticSystemDesignDocument;
pub use agentic_system_execution_view::AgenticSystemExecutionView;
pub use agentic_system_pin_document::AgenticSystemPinDocument;
pub use agentic_system_pins::AgenticSystemPins;
pub use agentic_system_publication_view::AgenticSystemPublicationView;
pub use agentic_system_validation_view::AgenticSystemValidationView;
pub use agentic_system_view::AgenticSystemView;
pub use ceremony_launcher::CeremonyLauncher;
pub use design_agentic_system_use_case::DesignAgenticSystemUseCase;
pub use get_agentic_system_execution_use_case::GetAgenticSystemExecutionUseCase;
pub use get_agentic_system_use_case::GetAgenticSystemUseCase;
pub use instantiate_agentic_system_input::InstantiateAgenticSystemInput;
pub use instantiate_agentic_system_use_case::InstantiateAgenticSystemUseCase;
pub use list_agentic_systems_use_case::ListAgenticSystemsUseCase;
pub use participant_offer::ParticipantOffer;
pub use publish_agentic_system_use_case::PublishAgenticSystemUseCase;
pub use render_agentic_system_diagram_use_case::RenderAgenticSystemDiagramUseCase;
pub use validate_agentic_system_use_case::ValidateAgenticSystemUseCase;
