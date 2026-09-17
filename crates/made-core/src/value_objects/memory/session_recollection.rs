use serde::{Deserialize, Serialize};

use super::{
    MemoryEntry, MemoryEntryKind, MemoryScope, RecalledEntry, RecollectionBudget,
    RecollectionCompleteness,
};

/// What earlier sessions in this scope decided, as one session reads it
/// when it opens.
///
/// A rendering, not a copy of the memory. Two rules make it one:
///
/// **Decisions and constraints first.** A session opening has a bound
/// on what it can carry, and what it carries first is what a later
/// session is most likely to act on: what was settled, and what was
/// ruled out. Observations and outcomes follow. Inside each group the
/// order is the order the entries came back in, so a backend that keeps
/// its writes in order stays readable as a sequence.
///
/// **Bounded, and it says when the bound bit.** The rendering stops at
/// the first entry that will not fit and reports itself truncated. An
/// entry too large for the whole budget is dropped rather than cut:
/// half a decision is a sentence that says something else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecollection {
    scope: MemoryScope,
    entries: Vec<RecalledEntry>,
    completeness: RecollectionCompleteness,
}

impl SessionRecollection {
    /// Render what a scope holds into what an opening can carry.
    #[must_use]
    pub fn rendered(
        scope: MemoryScope,
        recalled: &[MemoryEntry],
        budget: RecollectionBudget,
    ) -> Self {
        let ordered = recalled
            .iter()
            .filter(|entry| leads(entry.kind()))
            .chain(recalled.iter().filter(|entry| !leads(entry.kind())))
            .map(RecalledEntry::of);

        let mut entries = Vec::new();
        let mut spent = 0usize;
        let mut completeness = RecollectionCompleteness::Whole;
        for entry in ordered {
            match spent.checked_add(entry.cost()) {
                Some(total) if total <= budget.bytes() => {
                    spent = total;
                    entries.push(entry);
                }
                _ => {
                    completeness = RecollectionCompleteness::Truncated;
                    break;
                }
            }
        }

        Self {
            scope,
            entries,
            completeness,
        }
    }

    #[must_use]
    pub fn scope(&self) -> &MemoryScope {
        &self.scope
    }

    #[must_use]
    pub fn entries(&self) -> &[RecalledEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn completeness(&self) -> RecollectionCompleteness {
        self.completeness
    }

    /// Nothing came back. A session with nothing to recall records
    /// nothing, so its stream is the stream it would have had before
    /// memory could be read at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What a later session is most likely to act on.
const fn leads(kind: MemoryEntryKind) -> bool {
    matches!(
        kind,
        MemoryEntryKind::Decision | MemoryEntryKind::Constraint
    )
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use time::OffsetDateTime;

    use crate::value_objects::{Attributes, CeremonyId, MemoryEntryId, MemoryProvenance};

    use super::*;

    fn budget(bytes: usize) -> RecollectionBudget {
        RecollectionBudget::of_bytes(NonZeroUsize::new(bytes).expect("a budget of something"))
    }

    fn entry(id: &str, kind: MemoryEntryKind, summary: &str) -> MemoryEntry {
        MemoryEntry::new(
            MemoryEntryId::new(id).expect("a valid entry id"),
            kind,
            summary,
            MemoryProvenance::new(
                CeremonyId::new("earlier-session").expect("a valid ceremony id"),
                None,
                OffsetDateTime::UNIX_EPOCH,
            ),
            Attributes::empty(),
        )
        .expect("a valid entry")
    }

    fn scope() -> MemoryScope {
        MemoryScope::new("team:alpha").expect("a valid scope")
    }

    fn summaries(recollection: &SessionRecollection) -> Vec<&str> {
        recollection
            .entries()
            .iter()
            .map(RecalledEntry::summary)
            .collect()
    }

    #[test]
    fn decisions_and_constraints_come_first_and_the_rest_keep_their_order() {
        let recalled = vec![
            entry(
                "o1",
                MemoryEntryKind::Observation,
                "the queue was backing up",
            ),
            entry("d1", MemoryEntryKind::Decision, "roll back"),
            entry("x1", MemoryEntryKind::Outcome, "the queue drained"),
            entry(
                "c1",
                MemoryEntryKind::Constraint,
                "not during business hours",
            ),
        ];

        let rendered = SessionRecollection::rendered(scope(), &recalled, budget(4096));

        assert_eq!(
            summaries(&rendered),
            vec![
                "roll back",
                "not during business hours",
                "the queue was backing up",
                "the queue drained",
            ]
        );
        assert_eq!(rendered.completeness(), RecollectionCompleteness::Whole);
    }

    /// The bound is the point: what is dropped is what a later session
    /// was least likely to need, and it is told that something was.
    #[test]
    fn the_budget_stops_the_rendering_and_says_so() {
        let recalled = vec![
            entry("o1", MemoryEntryKind::Observation, "0123456789"),
            entry("d1", MemoryEntryKind::Decision, "0123456789"),
        ];

        let rendered = SessionRecollection::rendered(scope(), &recalled, budget(10));

        assert_eq!(summaries(&rendered), vec!["0123456789"]);
        assert_eq!(rendered.entries()[0].kind(), MemoryEntryKind::Decision);
        assert_eq!(rendered.completeness(), RecollectionCompleteness::Truncated);
        assert!(rendered.completeness().is_truncated());
    }

    /// Half a decision is a sentence that says something else, so an
    /// entry that cannot fit whole does not go in at all.
    #[test]
    fn an_entry_larger_than_the_whole_budget_is_dropped_rather_than_cut() {
        let recalled = vec![entry("d1", MemoryEntryKind::Decision, "0123456789")];

        let rendered = SessionRecollection::rendered(scope(), &recalled, budget(4));

        assert!(rendered.is_empty());
        assert_eq!(rendered.completeness(), RecollectionCompleteness::Truncated);
    }

    #[test]
    fn a_scope_that_holds_nothing_renders_to_nothing() {
        let rendered = SessionRecollection::rendered(scope(), &[], budget(4096));

        assert!(rendered.is_empty());
        assert_eq!(rendered.completeness(), RecollectionCompleteness::Whole);
        assert_eq!(rendered.scope().as_str(), "team:alpha");
    }

    #[test]
    fn a_recalled_entry_carries_the_session_that_said_it() {
        let recalled = vec![entry("d1", MemoryEntryKind::Decision, "roll back")];

        let rendered = SessionRecollection::rendered(scope(), &recalled, budget(4096));

        let only = &rendered.entries()[0];
        assert_eq!(only.id().as_str(), "d1");
        assert_eq!(only.from_ceremony().as_str(), "earlier-session");
        assert_eq!(only.observed_at(), OffsetDateTime::UNIX_EPOCH);
    }
}
