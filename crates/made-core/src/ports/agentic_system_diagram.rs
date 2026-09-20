//! [`AgenticSystemDiagramPort`] — the topology, drawn.

use crate::entities::{AgenticSystem, AgenticSystemExecution};
use crate::error::DomainError;

use super::AgenticSystemDiagram;

/// Render a design, and optionally one run of it, as a diagram.
///
/// A port rather than a method on the aggregate: Mermaid is a wire
/// format belonging to whoever displays it, and the domain has no
/// business knowing what an arrow looks like.
///
/// Synchronous, because drawing is a pure function of what it was
/// given. A renderer that needed to go and ask something would be
/// rendering a different picture than the one it was handed.
pub trait AgenticSystemDiagramPort: Send + Sync {
    fn render(
        &self,
        system: &AgenticSystem,
        execution: Option<&AgenticSystemExecution>,
    ) -> Result<AgenticSystemDiagram, DomainError>;
}
