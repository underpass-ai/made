use serde::{Deserialize, Serialize};

use super::AuditSequence;

/// How a journal failed verification.
///
/// Each variant names a distinct way a chain can be attacked, because
/// "the audit is broken" is not actionable and "the record at position
/// 7 no longer matches its digest" is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "defect")]
pub enum AuditChainDefect {
    /// The journal does not open at the first position — records were
    /// removed from the front.
    DoesNotStartAtTheBeginning { found: AuditSequence },

    /// A record's content no longer produces its own digest.
    DigestAltered { at: AuditSequence },

    /// The opening record claims a predecessor, or a later record
    /// claims none. Either way a link was rewritten.
    UnexpectedRoot { at: AuditSequence },

    /// A position was skipped or repeated — records were removed from
    /// the middle, or reordered.
    SequenceBroken {
        expected: AuditSequence,
        found: AuditSequence,
    },

    /// A record names a predecessor digest that is not the digest of
    /// the record before it. Something was substituted.
    LinkBroken { at: AuditSequence },

    /// A record belongs to a different ceremony — journals were
    /// grafted together.
    ForeignCeremony { at: AuditSequence },
}

impl AuditChainDefect {
    /// What went wrong, in words an operator can act on.
    ///
    /// Written once here rather than at each surface: the same defect
    /// has to read the same whether it was found over gRPC, in
    /// process, or by a host running the verifier itself.
    #[must_use]
    pub fn explain(self) -> String {
        match self {
            Self::DoesNotStartAtTheBeginning { found } => format!(
                "the journal opens at position {}, so records were removed from the front",
                found.value()
            ),
            Self::DigestAltered { at } => format!(
                "the record at position {} no longer produces its own digest",
                at.value()
            ),
            Self::UnexpectedRoot { at } => format!(
                "the record at position {} disagrees with the rest of the journal about \
                 whether it has a predecessor",
                at.value()
            ),
            Self::SequenceBroken { expected, found } => format!(
                "position {} was expected and position {} was found, so records were \
                 removed from the middle or reordered",
                expected.value(),
                found.value()
            ),
            Self::LinkBroken { at } => format!(
                "the record at position {} names a predecessor digest that is not the \
                 digest of the record before it",
                at.value()
            ),
            Self::ForeignCeremony { at } => format!(
                "the record at position {} belongs to another ceremony",
                at.value()
            ),
        }
    }

    /// Where the journal stopped being trustworthy.
    #[must_use]
    pub fn at(self) -> AuditSequence {
        match self {
            Self::DoesNotStartAtTheBeginning { found } | Self::SequenceBroken { found, .. } => {
                found
            }
            Self::DigestAltered { at }
            | Self::UnexpectedRoot { at }
            | Self::LinkBroken { at }
            | Self::ForeignCeremony { at } => at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(value: u64) -> AuditSequence {
        AuditSequence::new(value).unwrap()
    }

    #[test]
    fn every_defect_names_the_position_it_was_found_at() {
        for defect in [
            AuditChainDefect::DoesNotStartAtTheBeginning { found: at(3) },
            AuditChainDefect::DigestAltered { at: at(3) },
            AuditChainDefect::UnexpectedRoot { at: at(3) },
            AuditChainDefect::SequenceBroken {
                expected: at(2),
                found: at(3),
            },
            AuditChainDefect::LinkBroken { at: at(3) },
            AuditChainDefect::ForeignCeremony { at: at(3) },
        ] {
            assert_eq!(defect.at(), at(3), "{defect:?}");
            assert!(
                defect.explain().contains('3'),
                "{defect:?} explains itself without naming where: {}",
                defect.explain()
            );
        }
    }

    #[test]
    fn a_broken_sequence_names_both_positions() {
        let explanation = AuditChainDefect::SequenceBroken {
            expected: at(2),
            found: at(5),
        }
        .explain();

        assert!(explanation.contains('2'), "{explanation}");
        assert!(explanation.contains('5'), "{explanation}");
    }
}
