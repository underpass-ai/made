//! JSON projections shared by every surface that speaks JSON.
//!
//! The gRPC service and the in-process MCP backend both render the
//! same answers, and the contract says they must render them
//! identically. Two renderers would make that a property a test hopes
//! for; one renderer, reachable from both, makes it a property of the
//! code.

mod agentic_system_execution_json;
mod agentic_system_json;
mod agentic_system_validation_json;

pub use agentic_system_execution_json::AgenticSystemExecutionJson;
pub use agentic_system_json::AgenticSystemJson;
pub use agentic_system_validation_json::AgenticSystemValidationJson;
