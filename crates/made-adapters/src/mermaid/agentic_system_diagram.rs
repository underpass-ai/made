//! [`AgenticSystemMermaidDiagram`] — the topology as Mermaid text.
//!
//! The adapter renders; it never decides. Which arrow style means
//! which kind of collaboration is a domain fact the value object
//! carries, and this file only writes it down in Mermaid's spelling.
//!
//! Nothing here turns the text into an image. Rendering is the host's
//! job: shipping a browser engine to draw a box would cost more than
//! the box is worth, and every host that would display this can
//! already render Mermaid.

use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::error::DomainError;
use made_core::ports::{AgenticSystemDiagram, AgenticSystemDiagramPort};

mod flowchart;
mod text_equivalent;

/// Draws a design, and optionally one run of it.
#[derive(Debug, Default, Clone, Copy)]
pub struct AgenticSystemMermaidDiagram;

impl AgenticSystemMermaidDiagram {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl AgenticSystemDiagramPort for AgenticSystemMermaidDiagram {
    fn render(
        &self,
        system: &AgenticSystem,
        execution: Option<&AgenticSystemExecution>,
    ) -> Result<AgenticSystemDiagram, DomainError> {
        Ok(AgenticSystemDiagram::new(
            flowchart::render(system, execution),
            text_equivalent::render(system, execution),
        ))
    }
}
