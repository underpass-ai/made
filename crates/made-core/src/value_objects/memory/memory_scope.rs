use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::value_objects::CeremonyId;

const MAX_LENGTH: usize = 256;
const SEPARATOR: char = ':';

/// What a memory is about.
///
/// A scope is the thing memory is organised around — for this engine a
/// working session, for a host that keeps memory of its own something
/// else. It is a validated string rather than a ceremony id because
/// the engine writing memory and the memory itself do not have to
/// agree on what the world is made of, only on how to name a corner
/// of it.
///
/// # The grammar
///
/// ```text
/// scope := kind ":" name
/// kind  := one or more of [a-z0-9_-]
/// name  := non-empty, no control characters, colons allowed
/// ```
///
/// with the whole thing trimmed and at most 256 characters.
///
/// The kind is required, and it is the difference between a memory and
/// a collision. A scope stopped being derived from the session the
/// moment a definition could declare one: two hosts naming their
/// scopes for the things they care about — cases, tickets, teams —
/// share one memory with no way to tell whose entry is whose unless
/// each name says what kind of thing it is about. `ceremony:{id}`
/// already worked that way; this makes it the rule rather than the
/// habit of the one caller that formed a scope.
///
/// The name may itself contain colons, because a ceremony id may.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemoryScope(String);

impl MemoryScope {
    pub fn new(raw: impl Into<String>) -> Result<Self, DomainError> {
        let value = raw.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DomainError::EmptyField {
                field: "memory_scope",
            });
        }
        if trimmed.chars().count() > MAX_LENGTH {
            return Err(DomainError::FieldTooLong {
                field: "memory_scope",
                max: MAX_LENGTH,
                actual: trimmed.chars().count(),
            });
        }
        if trimmed.chars().any(char::is_control) {
            return Err(DomainError::InvalidCharacters {
                field: "memory_scope",
            });
        }
        let Some((kind, name)) = trimmed.split_once(SEPARATOR) else {
            return Err(DomainError::InvariantViolated {
                reason: "a memory scope says what kind of thing it is about, as `kind:name`",
            });
        };
        if kind.is_empty() || !kind.chars().all(is_kind_character) {
            return Err(DomainError::InvalidCharacters {
                field: "memory_scope.kind",
            });
        }
        if name.trim().is_empty() {
            return Err(DomainError::EmptyField {
                field: "memory_scope.name",
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The scope a working session's memory belongs to when the
    /// definition declares none.
    ///
    /// One session, one scope, which means nothing is shared with any
    /// other: this is what "no shared memory" looks like, and a
    /// ceremony that wants a memory a later one can read declares the
    /// scope it wants.
    pub fn of_ceremony(ceremony_id: &CeremonyId) -> Result<Self, DomainError> {
        Self::new(format!("ceremony{SEPARATOR}{}", ceremony_id.as_str()))
    }

    /// What kind of thing this memory is about.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.0.split_once(SEPARATOR).map_or("", |(kind, _)| kind)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Lowercase ASCII, digits, `_` and `-`. No colon, because the colon is
/// what ends the kind, and no case, because a memory shared between two
/// writers that disagree about capitalisation is two memories.
const fn is_kind_character(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || matches!(character, '_' | '-')
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_scope_is_the_ceremony_under_its_kind() {
        let scope = MemoryScope::of_ceremony(&CeremonyId::new("session-1").unwrap()).unwrap();

        assert_eq!(scope.as_str(), "ceremony:session-1");
        assert_eq!(scope.kind(), "ceremony");
    }

    /// A ceremony id may carry colons, so the name may too. Only the
    /// first colon means anything.
    #[test]
    fn only_the_first_colon_ends_the_kind() {
        let scope = MemoryScope::of_ceremony(&CeremonyId::new("run:2026:07").unwrap()).unwrap();

        assert_eq!(scope.kind(), "ceremony");
        assert_eq!(scope.as_str(), "ceremony:run:2026:07");
    }

    #[test]
    fn a_host_names_its_own_kinds() {
        for raw in ["team:alpha", "case-file:7", "board_room:north"] {
            assert_eq!(MemoryScope::new(raw).unwrap().as_str(), raw);
        }
    }

    /// The failure the kind exists to prevent, refused at the door: a
    /// bare name shares a namespace with every other bare name.
    #[test]
    fn a_scope_without_a_kind_is_refused() {
        let error = MemoryScope::new("alpha").unwrap_err();

        assert!(
            matches!(error, DomainError::InvariantViolated { .. }),
            "{error}"
        );
    }

    #[test]
    fn a_kind_that_is_not_lowercase_ascii_is_refused() {
        for raw in ["Team:alpha", "team name:alpha", "équipe:alpha", ":alpha"] {
            assert!(MemoryScope::new(raw).is_err(), "{raw} was accepted");
        }
    }

    #[test]
    fn a_kind_with_nothing_after_it_is_refused() {
        for raw in ["team:", "team:   "] {
            assert!(MemoryScope::new(raw).is_err(), "{raw} was accepted");
        }
    }

    #[test]
    fn an_empty_or_oversized_or_control_bearing_scope_is_refused() {
        assert!(MemoryScope::new("   ").is_err());
        assert!(MemoryScope::new(format!("team:{}", "a".repeat(MAX_LENGTH))).is_err());
        assert!(MemoryScope::new("team:al\npha").is_err());
    }
}
