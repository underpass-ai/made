use made_core::entities::{AgenticSystem, AgenticSystemValidationReport};
use made_core::value_objects::{AgenticSystemValidationFinding, DefinitionPin, SystemCeremonyId};
use std::collections::BTreeMap;

/// What the analysis found, and what it was performed against.
///
/// The resolved pins are part of the answer rather than a detail of
/// how it was reached: "publishable" is a statement about one set of
/// published definitions, and a report that did not name them would
/// be an opinion about documents nobody could look up.
#[derive(Debug, Clone, PartialEq)]
pub struct AgenticSystemValidationView {
    system: AgenticSystem,
    report: AgenticSystemValidationReport,
}

impl AgenticSystemValidationView {
    #[must_use]
    pub const fn new(system: AgenticSystem, report: AgenticSystemValidationReport) -> Self {
        Self { system, report }
    }

    #[must_use]
    pub const fn system(&self) -> &AgenticSystem {
        &self.system
    }

    #[must_use]
    pub fn findings(&self) -> &[AgenticSystemValidationFinding] {
        self.report.findings()
    }

    #[must_use]
    pub const fn resolved_pins(&self) -> &BTreeMap<SystemCeremonyId, DefinitionPin> {
        self.report.resolved_pins()
    }

    #[must_use]
    pub fn is_publishable(&self) -> bool {
        self.report.is_publishable()
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.report.errors().count()
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.report.warnings().count()
    }

    #[must_use]
    pub const fn report(&self) -> &AgenticSystemValidationReport {
        &self.report
    }
}
