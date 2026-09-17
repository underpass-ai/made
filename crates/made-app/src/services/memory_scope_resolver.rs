//! Which memory a working session reads and writes.
//!
//! One pure function, because the answer has to be the same in two
//! places that run at different times: the recall that happens when a
//! session opens, and every write the session makes afterwards. A scope
//! decided twice is a session that reads one memory and writes another,
//! which looks exactly like a memory that forgets.
//!
//! The scope is resolved from the session rather than stored on it. It
//! follows from the context the session was started with, the context
//! is in the opening event, and the opening event is the first thing
//! any fold has — so a second field carrying the same fact could only
//! ever disagree with it.

use made_core::entities::CeremonyInstance;
use made_core::error::DomainError;
use made_core::value_objects::{CeremonyContext, CeremonyId, MemoryScope};
use serde_json::Value;

/// The reserved context key a definition uses to declare its scope.
///
/// Reserved: the engine reads this one key of an otherwise opaque
/// context. Everything else in there stays the host's.
pub const MEMORY_SCOPE: &str = "memory_scope";

/// The scope this session's memory belongs to.
pub fn of_instance(instance: &CeremonyInstance) -> Result<MemoryScope, DomainError> {
    of_context(instance.context(), instance.id())
}

/// The scope a session started with this context belongs to.
///
/// Declared, it is what the context says. Absent, it is the session's
/// own id, which means **no shared memory**: nothing written under it
/// is reachable from any other session, because no other session ever
/// looks there.
///
/// A key that is present and is not a usable scope is refused rather
/// than replaced by the default. An operator who asked two sessions to
/// share a memory and silently got two private ones would have no way
/// to tell that from a memory that lost the entries.
pub fn of_context(
    context: &CeremonyContext,
    ceremony_id: &CeremonyId,
) -> Result<MemoryScope, DomainError> {
    match context.attributes().get(MEMORY_SCOPE) {
        None => MemoryScope::of_ceremony(ceremony_id),
        Some(Value::String(declared)) => MemoryScope::new(declared.clone()),
        Some(_) => Err(DomainError::InvalidCharacters {
            field: "context.memory_scope",
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use made_core::entities::ceremony_events::CeremonyInstanceStarted;
    use made_core::value_objects::{Attributes, CeremonyName, CeremonyVersion, StateId};
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;

    fn ceremony_id() -> CeremonyId {
        CeremonyId::new("session-1").expect("a valid ceremony id")
    }

    fn context(value: Value) -> CeremonyContext {
        CeremonyContext::new(
            Attributes::new([(MEMORY_SCOPE.to_owned(), value)].into_iter().collect())
                .expect("valid attributes"),
        )
    }

    #[test]
    fn a_session_that_declares_nothing_remembers_alone() {
        let scope = of_context(&CeremonyContext::empty(), &ceremony_id()).unwrap();

        assert_eq!(scope.as_str(), "ceremony:session-1");
    }

    #[test]
    fn a_declared_scope_is_what_the_context_says() {
        let scope = of_context(&context(json!("team:alpha")), &ceremony_id()).unwrap();

        assert_eq!(scope.as_str(), "team:alpha");
    }

    #[test]
    fn a_declared_scope_that_is_not_a_scope_is_refused() {
        for declared in [json!("alpha"), json!(""), json!("Team:alpha")] {
            assert!(
                of_context(&context(declared.clone()), &ceremony_id()).is_err(),
                "{declared} was accepted as a scope"
            );
        }
    }

    /// Refused rather than ignored: a context whose `memory_scope` is a
    /// number is a caller that meant something by it.
    #[test]
    fn a_declared_scope_that_is_not_a_string_is_refused() {
        let error = of_context(&context(json!(7)), &ceremony_id()).unwrap_err();

        assert!(
            matches!(
                error,
                DomainError::InvalidCharacters {
                    field: "context.memory_scope"
                }
            ),
            "{error}"
        );
    }

    /// The two halves of one answer: what a session writes goes where
    /// the next session in that scope will look.
    #[test]
    fn an_instance_resolves_to_the_scope_its_context_declared() {
        let instance = CeremonyInstance::from_started(&CeremonyInstanceStarted {
            ceremony_id: ceremony_id(),
            definition_name: CeremonyName::new("session_memory").expect("a valid name"),
            definition_version: CeremonyVersion::v1(),
            initial_state: StateId::new("OPEN").expect("a valid state"),
            step_ids: BTreeSet::new(),
            context: context(json!("team:alpha")),
            bound_definition: None,
            created_at: OffsetDateTime::UNIX_EPOCH,
        });

        assert_eq!(of_instance(&instance).unwrap().as_str(), "team:alpha");
    }
}
