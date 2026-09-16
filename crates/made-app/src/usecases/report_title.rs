use made_core::error::DomainError;

/// What a caller wants the report called.
///
/// A heading is the one part of a report a caller writes, and what
/// counts as one was decided twice: the in-process arm trimmed the
/// string before it reached the engine and the gRPC arm did not, so
/// `" Session review "` rendered two documents from one request. The
/// rule belongs to the value, not to whichever mapper the call happened
/// to pass through, so both arms build this and neither decides
/// anything.
///
/// Trimmed, because surrounding space is never part of what anyone
/// meant to call something. Refused when nothing is left, because a
/// heading nobody can read is not a choice anyone made — and an absent
/// title already has a meaning, the default heading, which a blank one
/// would silently borrow.
///
/// Escaping stays where it was: `safe_heading` treats the text as the
/// untrusted Markdown it is when the document is rendered. This says
/// what a title *is*; that says what a document may contain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportTitle(String);

impl ReportTitle {
    /// The caller's heading, trimmed. Blank is refused.
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField { field: "title" });
        }
        Ok(Self(trimmed))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ReportTitle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrounding_space_is_not_part_of_what_anyone_meant() {
        assert_eq!(
            ReportTitle::new("  Session review  ")
                .expect("a padded title is a title")
                .as_str(),
            "Session review"
        );
    }

    /// The whole point of the type: the same raw string is the same
    /// heading whichever arm read it.
    #[test]
    fn one_padded_string_is_one_heading() {
        assert_eq!(
            ReportTitle::new(" Session review "),
            ReportTitle::new("Session review")
        );
    }

    #[test]
    fn a_heading_nobody_can_read_is_refused() {
        for blank in ["", "   ", "\t\n"] {
            assert!(
                matches!(
                    ReportTitle::new(blank),
                    Err(DomainError::EmptyField { field: "title" })
                ),
                "{blank:?} is not a heading"
            );
        }
    }

    #[test]
    fn inner_space_is_the_callers_own() {
        assert_eq!(
            ReportTitle::new("Session   review").unwrap().as_str(),
            "Session   review"
        );
    }
}
