//! [`ProtocolDraftView`] — what a draft is and what is wrong with it.
//!
//! Shared by every distribution for the same reason the instance view
//! is: an author asking "is this publishable, and why not" must get
//! the same answer whichever way they reached the engine. Counting a
//! draft's parts and explaining its defects are decisions about what
//! the answer *is*, not about how to encode it, so they belong here
//! and the encoding belongs to each transport.

use made_core::entities::ProtocolDefinitionDraft;
use made_core::value_objects::{
    ProtocolName, ProtocolValidationFinding, ProtocolValidationReport, ProtocolVersion,
};

use super::protocol_draft_summary::ProtocolDraftSummary;

pub struct ProtocolDraftView<'a> {
    draft: &'a ProtocolDefinitionDraft,
    report: &'a ProtocolValidationReport,
    summary: ProtocolDraftSummary,
    narrative: Vec<String>,
}

impl std::fmt::Debug for ProtocolDraftView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolDraftView")
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}

impl<'a> ProtocolDraftView<'a> {
    #[must_use]
    pub fn project(
        draft: &'a ProtocolDefinitionDraft,
        report: &'a ProtocolValidationReport,
    ) -> Self {
        let summary = ProtocolDraftSummary::new(
            draft.states().len(),
            draft
                .states()
                .iter()
                .filter(|state| state.is_initial())
                .count(),
            draft
                .states()
                .iter()
                .filter(|state| state.is_terminal())
                .count(),
            draft.transitions().len(),
            draft.steps().len(),
            draft.guards().len(),
            draft.roles().len(),
        );
        let narrative = narrate(draft, report, &summary);
        Self {
            draft,
            report,
            summary,
            narrative,
        }
    }

    #[must_use]
    pub fn name(&self) -> &ProtocolName {
        self.draft.name()
    }

    #[must_use]
    pub fn version(&self) -> &ProtocolVersion {
        self.draft.version()
    }

    /// Whether anything blocks publication. A draft with warnings and
    /// no errors is publishable and still worth reading about.
    #[must_use]
    pub fn is_publishable(&self) -> bool {
        self.report.is_valid()
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
    pub fn findings(&self) -> &[ProtocolValidationFinding] {
        self.report.findings()
    }

    #[must_use]
    pub const fn summary(&self) -> ProtocolDraftSummary {
        self.summary
    }

    /// The explanation, in the order it should be read: what the draft
    /// is, then what stops it, then what merely troubles it.
    #[must_use]
    pub fn narrative(&self) -> &[String] {
        &self.narrative
    }
}

fn narrate(
    draft: &ProtocolDefinitionDraft,
    report: &ProtocolValidationReport,
    summary: &ProtocolDraftSummary,
) -> Vec<String> {
    let mut lines = vec![format!(
        "`{}` declares {} states, {} transitions, {} steps, {} guards and {} roles.",
        draft.name().as_str(),
        summary.states(),
        summary.transitions(),
        summary.steps(),
        summary.guards(),
        summary.roles(),
    )];

    let errors = report.errors().collect::<Vec<_>>();
    if errors.is_empty() {
        lines.push("Nothing blocks publication.".to_owned());
    } else {
        lines.push(format!(
            "{} defect(s) block publication; the draft cannot be published or executed until every one is fixed.",
            errors.len()
        ));
        lines.extend(errors.into_iter().map(describe));
    }

    let warnings = report.warnings().collect::<Vec<_>>();
    if !warnings.is_empty() {
        lines.push(format!(
            "{} warning(s) do not block publication but describe a protocol that can stall.",
            warnings.len()
        ));
        lines.extend(warnings.into_iter().map(describe));
    }

    lines
}

fn describe(finding: &ProtocolValidationFinding) -> String {
    format!("{} — at {}", finding.defect(), finding.locus())
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::value_objects::{ProtocolState, ProtocolTransition, StateId, TransitionTrigger};

    fn state(id: &str) -> StateId {
        StateId::new(id).unwrap()
    }

    /// A draft whose only transition leads somewhere it never declared.
    fn broken_draft() -> ProtocolDefinitionDraft {
        ProtocolDefinitionDraft::new(
            ProtocolName::new("broken_protocol").unwrap(),
            ProtocolVersion::v1(),
            None,
            Vec::new(),
            Vec::new(),
            vec![
                ProtocolState::initial(state("DRAFTING")),
                ProtocolState::terminal(state("DONE")),
            ],
            vec![ProtocolTransition::new(
                state("DRAFTING"),
                state("NOWHERE"),
                TransitionTrigger::new("finish").unwrap(),
                Vec::new(),
            )
            .unwrap()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn the_first_line_says_what_the_draft_is() {
        let draft = broken_draft();
        let report = draft.analyze();
        let view = ProtocolDraftView::project(&draft, &report);

        assert_eq!(
            view.narrative()[0],
            "`broken_protocol` declares 2 states, 1 transitions, 0 steps, 0 guards and 0 roles."
        );
        assert_eq!(view.summary().states(), 2);
        assert_eq!(view.summary().initial_states(), 1);
        assert_eq!(view.summary().terminal_states(), 1);
    }

    #[test]
    fn a_blocked_draft_says_so_and_names_each_element() {
        let draft = broken_draft();
        let report = draft.analyze();
        let view = ProtocolDraftView::project(&draft, &report);

        assert!(!view.is_publishable());
        assert!(view.error_count() > 0);
        assert!(view
            .narrative()
            .iter()
            .any(|line| line.contains("block publication")));
        // Each defect points at an element in words, not as a
        // serialized object quoted into the middle of a sentence.
        assert!(view
            .narrative()
            .iter()
            .any(|line| line.contains(" — at ") && !line.contains('{')));
    }
}
