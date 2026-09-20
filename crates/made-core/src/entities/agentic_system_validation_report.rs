use std::collections::BTreeMap;

use crate::value_objects::{AgenticSystemValidationFinding, DefinitionPin, SystemCeremonyId};

/// The complete outcome of analysing an agentic system design.
///
/// Every defect, not the first one: a design has a dozen kinds of
/// element that can disagree with each other, and an author told about
/// one at a time would need a dozen round trips to get a publishable
/// system.
///
/// The resolved pins travel with the report because they are what the
/// analysis was performed against. A report that said "publishable"
/// without saying which published definitions it checked would be an
/// opinion about an unnamed set of documents.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgenticSystemValidationReport {
    findings: Vec<AgenticSystemValidationFinding>,
    resolved_pins: BTreeMap<SystemCeremonyId, DefinitionPin>,
}

impl AgenticSystemValidationReport {
    #[must_use]
    pub fn new(
        findings: impl IntoIterator<Item = AgenticSystemValidationFinding>,
        resolved_pins: impl IntoIterator<Item = (SystemCeremonyId, DefinitionPin)>,
    ) -> Self {
        Self {
            findings: findings.into_iter().collect(),
            resolved_pins: resolved_pins.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn findings(&self) -> &[AgenticSystemValidationFinding] {
        &self.findings
    }

    #[must_use]
    pub const fn resolved_pins(&self) -> &BTreeMap<SystemCeremonyId, DefinitionPin> {
        &self.resolved_pins
    }

    pub fn errors(&self) -> impl Iterator<Item = &AgenticSystemValidationFinding> {
        self.findings.iter().filter(|finding| finding.is_blocking())
    }

    pub fn warnings(&self) -> impl Iterator<Item = &AgenticSystemValidationFinding> {
        self.findings
            .iter()
            .filter(|finding| !finding.is_blocking())
    }

    /// The first blocking finding, in check order.
    #[must_use]
    pub fn first_error(&self) -> Option<&AgenticSystemValidationFinding> {
        self.findings.iter().find(|finding| finding.is_blocking())
    }

    /// A design may be published only when no finding blocks it.
    #[must_use]
    pub fn is_publishable(&self) -> bool {
        self.first_error().is_none()
    }

    /// Every blocking finding on one line, for an error envelope that
    /// has room for a sentence rather than a structure.
    #[must_use]
    pub fn blocking_summary(&self) -> String {
        self.errors()
            .map(AgenticSystemValidationFinding::explain)
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[cfg(test)]
mod tests {
    use crate::value_objects::AgenticSystemValidationLocus;

    use super::*;

    #[test]
    fn an_empty_report_is_publishable_and_says_nothing() {
        let report = AgenticSystemValidationReport::default();

        assert!(report.is_publishable());
        assert!(report.blocking_summary().is_empty());
    }

    #[test]
    fn every_blocking_finding_reaches_the_summary() {
        let report = AgenticSystemValidationReport::new(
            [
                AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::System,
                    "first",
                ),
                AgenticSystemValidationFinding::warning(
                    AgenticSystemValidationLocus::System,
                    crate::error::DomainError::InvariantViolated { reason: "advice" },
                ),
                AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::System,
                    "second",
                ),
            ],
            [],
        );

        assert!(!report.is_publishable());
        assert_eq!(report.errors().count(), 2);
        assert_eq!(report.warnings().count(), 1);
        assert!(report.blocking_summary().contains("first"));
        assert!(report.blocking_summary().contains("second"));
    }
}
