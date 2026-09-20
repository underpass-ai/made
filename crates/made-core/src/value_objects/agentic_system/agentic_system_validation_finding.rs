use crate::error::DomainError;
use crate::value_objects::CeremonyValidationSeverity;

use super::AgenticSystemValidationLocus;

/// One defect found while analysing an agentic system design.
///
/// The severity vocabulary is the ceremony one, because "error blocks
/// publication, warning does not" means the same thing at both levels
/// and a second vocabulary would only need translating. The locus is
/// its own, because a design has elements a definition does not.
///
/// Deliberately not serializable: it carries a [`DomainError`], and
/// serialization belongs to the adapter that renders it.
#[derive(Debug, Clone, PartialEq)]
pub struct AgenticSystemValidationFinding {
    severity: CeremonyValidationSeverity,
    locus: AgenticSystemValidationLocus,
    defect: DomainError,
}

impl AgenticSystemValidationFinding {
    #[must_use]
    pub const fn error(locus: AgenticSystemValidationLocus, defect: DomainError) -> Self {
        Self {
            severity: CeremonyValidationSeverity::Error,
            locus,
            defect,
        }
    }

    #[must_use]
    pub const fn warning(locus: AgenticSystemValidationLocus, defect: DomainError) -> Self {
        Self {
            severity: CeremonyValidationSeverity::Warning,
            locus,
            defect,
        }
    }

    /// An error whose explanation names the elements involved.
    #[must_use]
    pub fn refusal(locus: AgenticSystemValidationLocus, reason: impl Into<String>) -> Self {
        Self::error(
            locus,
            DomainError::InvalidDocument {
                reason: reason.into(),
            },
        )
    }

    #[must_use]
    pub const fn severity(&self) -> CeremonyValidationSeverity {
        self.severity
    }

    #[must_use]
    pub const fn locus(&self) -> &AgenticSystemValidationLocus {
        &self.locus
    }

    #[must_use]
    pub const fn defect(&self) -> &DomainError {
        &self.defect
    }

    #[must_use]
    pub fn is_blocking(&self) -> bool {
        self.severity.is_error()
    }

    /// The finding as one line: where, then what.
    #[must_use]
    pub fn explain(&self) -> String {
        format!("{}: {}", self.locus, self.defect)
    }
}

#[cfg(test)]
mod tests {
    use crate::value_objects::agentic_system::SystemCeremonyId;

    use super::*;

    #[test]
    fn a_finding_says_where_before_it_says_what() {
        let finding = AgenticSystemValidationFinding::refusal(
            AgenticSystemValidationLocus::ceremony(SystemCeremonyId::new("review").unwrap()),
            "no published definition matches the pin",
        );

        assert!(finding.is_blocking());
        assert!(finding.explain().starts_with("ceremony `review`: "));
        assert!(finding.explain().contains("matches the pin"));
    }
}
