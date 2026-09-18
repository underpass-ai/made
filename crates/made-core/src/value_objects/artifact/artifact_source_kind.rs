use serde::{Deserialize, Serialize};

/// What produced an artifact or execution receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSourceKind {
    ExternalExecution,
    Fixture,
    NoOp,
    GeneratedReport,
    Imported,
}

impl ArtifactSourceKind {
    /// Whether this source may be produced by a step execution connector.
    #[must_use]
    pub const fn is_execution_source(self) -> bool {
        matches!(self, Self::ExternalExecution | Self::Fixture | Self::NoOp)
    }
}
