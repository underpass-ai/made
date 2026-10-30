use super::ProtocolReportBinding;

/// A report: the Markdown projection of persisted state, and what it
/// was projected from (ADR-006).
///
/// The counts are derived here rather than carried, because a count
/// that disagrees with the bindings beside it is a report nobody can
/// trust — and both arms read them from this one place, so they cannot
/// disagree with each other either.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolReport {
    markdown: String,
    bindings: Vec<ProtocolReportBinding>,
}

impl ProtocolReport {
    #[must_use]
    pub fn new(markdown: impl Into<String>, bindings: Vec<ProtocolReportBinding>) -> Self {
        Self {
            markdown: markdown.into(),
            bindings,
        }
    }

    #[must_use]
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The sessions reported, in the order the caller asked for them.
    #[must_use]
    pub fn bindings(&self) -> &[ProtocolReportBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn protocol_count(&self) -> usize {
        self.bindings.len()
    }

    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.bindings
            .iter()
            .filter(|binding| binding.completed())
            .count()
    }

    #[must_use]
    pub fn incomplete_count(&self) -> usize {
        self.protocol_count() - self.completed_count()
    }
}
