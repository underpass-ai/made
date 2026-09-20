//! [`AgenticSystemAnalysis`] — one pass over a design, against the
//! ceremonies that are actually published.
//!
//! The analysis is the only thing standing between a design somebody
//! wrote and a run that would discover its defects one stall at a
//! time. It therefore collects every finding rather than stopping at
//! the first, and every finding names the element it is about, so an
//! author — human or agent — can fix a design in one pass.
//!
//! It resolves nothing itself. The pins are looked up by the
//! application through the publication port and handed in, so the
//! domain stays a function of what it was given.

use std::collections::BTreeMap;

use crate::value_objects::SystemCeremonyId;

use super::{
    AgenticSystem, AgenticSystemParts, AgenticSystemValidationReport, PublishedCeremonyDefinition,
};

mod capabilities;
mod compositions;
mod dependencies;
mod independence;
mod references;

/// A design, the published definitions it pinned, and the checks that
/// compare them.
#[derive(Debug, Clone, Copy)]
pub struct AgenticSystemAnalysis<'design> {
    parts: AgenticSystemParts<'design>,
    resolved: &'design BTreeMap<SystemCeremonyId, PublishedCeremonyDefinition>,
}

impl<'design> AgenticSystemAnalysis<'design> {
    #[must_use]
    pub fn of(
        design: &'design AgenticSystem,
        resolved: &'design BTreeMap<SystemCeremonyId, PublishedCeremonyDefinition>,
    ) -> Self {
        Self {
            parts: AgenticSystemParts::of(design),
            resolved,
        }
    }

    /// Every defect, in check order.
    ///
    /// References first, because a check that reasoned about a name
    /// nothing declares would produce a second finding about the same
    /// mistake in a vocabulary the author never used.
    #[must_use]
    pub fn report(&self) -> AgenticSystemValidationReport {
        let mut findings = Vec::new();
        references::collect(self.parts, &mut findings);
        dependencies::collect(self.parts, &mut findings);
        compositions::collect(self.parts, self.resolved, &mut findings);
        capabilities::collect(self.parts, &mut findings);
        independence::collect(self.parts, &mut findings);
        AgenticSystemValidationReport::new(
            findings,
            self.resolved.iter().map(|(ceremony, published)| {
                (
                    ceremony.clone(),
                    crate::value_objects::DefinitionPin::new(
                        published.name().clone(),
                        published.version().clone(),
                        published.digest(),
                    ),
                )
            }),
        )
    }
}

impl AgenticSystem {
    /// Analyse this design against the definitions its pins resolved
    /// to.
    #[must_use]
    pub fn analyze(
        &self,
        resolved: &BTreeMap<SystemCeremonyId, PublishedCeremonyDefinition>,
    ) -> AgenticSystemValidationReport {
        AgenticSystemAnalysis::of(self, resolved).report()
    }
}
