//! Shared string hygiene for the design vocabulary.
//!
//! Every identifier in an agentic system design is written by a person
//! or an agent authoring a document, so the same two rules apply
//! everywhere: an identifier is a lower snake-case slug, and prose is
//! trimmed, bounded and free of control characters. Written once so a
//! new value object cannot quietly adopt looser rules than its
//! neighbours.

use crate::error::DomainError;

/// A `lower_snake_case` identifier, hyphens and digits allowed.
pub(super) fn slug(field: &'static str, raw: &str, max: usize) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    if trimmed.len() > max {
        return Err(DomainError::FieldTooLong {
            field,
            actual: trimmed.len(),
            max,
        });
    }
    if trimmed
        .chars()
        .any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-'))
    {
        return Err(DomainError::InvalidCharacters { field });
    }
    Ok(trimmed.to_owned())
}

/// Bounded prose: trimmed, non-empty, no control characters.
pub(super) fn text(field: &'static str, raw: &str, max: usize) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(DomainError::EmptyField { field });
    }
    if trimmed.len() > max {
        return Err(DomainError::FieldTooLong {
            field,
            actual: trimmed.len(),
            max,
        });
    }
    if trimmed.chars().any(char::is_control) {
        return Err(DomainError::InvalidCharacters { field });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slug_refuses_what_prose_allows() {
        assert!(slug("f", "reviewing_team", 64).is_ok());
        assert!(slug("f", "Reviewing Team", 64).is_err());
        assert!(text("f", "Reviewing Team", 64).is_ok());
        assert!(text("f", "line\nbreak", 64).is_err());
        assert!(text("f", "  ", 64).is_err());
        assert!(slug("f", "toolong", 3).is_err());
    }
}
