use crate::error::DomainError;

use super::{ProtocolValidationLocus, ProtocolValidationSeverity};

/// One defect found while analysing a protocol definition.
///
/// The defect keeps the typed [`DomainError`] that describes it, so a
/// blocking finding can be surfaced as the exact error a caller would
/// have received from fail-fast construction.
///
/// Deliberately not serializable: it carries a [`DomainError`], and
/// serialization belongs to the adapter layer. An adapter renders the
/// finding into its own wire shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolValidationFinding {
    severity: ProtocolValidationSeverity,
    locus: ProtocolValidationLocus,
    defect: DomainError,
}

impl ProtocolValidationFinding {
    #[must_use]
    pub fn error(locus: ProtocolValidationLocus, defect: DomainError) -> Self {
        Self {
            severity: ProtocolValidationSeverity::Error,
            locus,
            defect,
        }
    }

    #[must_use]
    pub fn warning(locus: ProtocolValidationLocus, defect: DomainError) -> Self {
        Self {
            severity: ProtocolValidationSeverity::Warning,
            locus,
            defect,
        }
    }

    #[must_use]
    pub fn severity(&self) -> ProtocolValidationSeverity {
        self.severity
    }

    #[must_use]
    pub fn locus(&self) -> &ProtocolValidationLocus {
        &self.locus
    }

    #[must_use]
    pub fn defect(&self) -> &DomainError {
        &self.defect
    }

    #[must_use]
    pub fn is_blocking(&self) -> bool {
        self.severity.is_error()
    }
}
