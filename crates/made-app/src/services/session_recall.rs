//! What a session reads before it starts.
//!
//! # Memory is not the transaction
//!
//! The same doctrine the recorder states, in the other direction: a
//! session that cannot read what earlier ones decided still runs. A
//! memory backend that is unreachable, or that answers with an error,
//! costs the session its recollection and nothing else — it is logged
//! at warning level with the scope it was asked about, and the session
//! opens without one.
//!
//! The alternative would be a start that fails because something
//! optional was unavailable, which trades a session somebody wants for
//! a convenience they would rather do without.

use made_core::ports::MemoryReaderPort;
use made_core::value_objects::{MemoryScope, RecollectionBudget, SessionRecollection};
use std::num::NonZeroUsize;

/// How much of a scope one opening carries, in bytes of summary.
///
/// Four kibibytes. A memory entry's summary is capped at 2048
/// characters, so the budget admits two of the longest entries anyone
/// can write and a dozen of the length sessions actually produce, and
/// it is small enough that the fact carrying it is an ordinary row in
/// the store rather than a reason to think about storage.
///
/// Declared once, here, because a bound that two call sites chose
/// separately is two bounds.
const RECOLLECTION_BUDGET: RecollectionBudget =
    RecollectionBudget::of_bytes(match NonZeroUsize::new(4_096) {
        Some(bytes) => bytes,
        None => unreachable!(),
    });

/// What the scope holds, rendered for an opening — or nothing.
///
/// `None` covers the three ways there is nothing to say: no memory
/// configured (the backend declares nothing and answers unsupported), a
/// scope nobody has written to, and a read that failed. A caller cannot
/// tell them apart and does not need to: in all three the session opens
/// having been told nothing, which is exactly what it records.
pub(crate) async fn recall(
    memory: &dyn MemoryReaderPort,
    scope: &MemoryScope,
) -> Option<SessionRecollection> {
    match memory.recall(scope).await {
        Ok(recalled) => {
            let rendered = SessionRecollection::rendered(
                scope.clone(),
                recalled.entries(),
                RECOLLECTION_BUDGET,
            );
            (!rendered.is_empty()).then_some(rendered)
        }
        Err(error) => {
            tracing::warn!(
                scope = scope.as_str(),
                %error,
                "a session could not read what earlier ones decided; it opens without a \
                 recollection and is otherwise unaffected"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use made_core::error::DomainError;
    use made_core::ports::MemoryRecollection;
    use made_core::value_objects::{
        Attributes, CeremonyId, MemoryCapabilities, MemoryEntry, MemoryEntryId, MemoryEntryKind,
        MemoryMoment, MemoryProvenance,
    };
    use time::OffsetDateTime;

    use super::*;

    struct MemoryThatAnswers(MemoryRecollection);
    struct MemoryThatIsOut;

    #[async_trait]
    impl MemoryReaderPort for MemoryThatAnswers {
        async fn recall(&self, _scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
            Ok(self.0.clone())
        }

        async fn as_known_at(
            &self,
            _scope: &MemoryScope,
            _moment: MemoryMoment,
        ) -> Result<MemoryRecollection, DomainError> {
            Ok(MemoryRecollection::Unsupported)
        }

        async fn follow(
            &self,
            _scope: &MemoryScope,
            _from: &MemoryEntryId,
            _to: &MemoryEntryId,
        ) -> Result<MemoryRecollection, DomainError> {
            Ok(MemoryRecollection::Unsupported)
        }

        fn capabilities(&self) -> MemoryCapabilities {
            MemoryCapabilities::none()
        }
    }

    #[async_trait]
    impl MemoryReaderPort for MemoryThatIsOut {
        async fn recall(&self, _scope: &MemoryScope) -> Result<MemoryRecollection, DomainError> {
            Err(DomainError::InvalidDocument {
                reason: "the memory backend did not answer".to_owned(),
            })
        }

        async fn as_known_at(
            &self,
            _scope: &MemoryScope,
            _moment: MemoryMoment,
        ) -> Result<MemoryRecollection, DomainError> {
            Ok(MemoryRecollection::Unsupported)
        }

        async fn follow(
            &self,
            _scope: &MemoryScope,
            _from: &MemoryEntryId,
            _to: &MemoryEntryId,
        ) -> Result<MemoryRecollection, DomainError> {
            Ok(MemoryRecollection::Unsupported)
        }

        fn capabilities(&self) -> MemoryCapabilities {
            MemoryCapabilities::none()
        }
    }

    fn scope() -> MemoryScope {
        MemoryScope::new("team:alpha").expect("a valid scope")
    }

    fn decision() -> MemoryEntry {
        MemoryEntry::new(
            MemoryEntryId::new("guard:sign_off").expect("a valid entry id"),
            MemoryEntryKind::Decision,
            "`sign_off` was approved",
            MemoryProvenance::new(
                CeremonyId::new("earlier-session").expect("a valid ceremony id"),
                None,
                OffsetDateTime::UNIX_EPOCH,
            ),
            Attributes::empty(),
        )
        .expect("a valid entry")
    }

    #[tokio::test]
    async fn what_the_scope_holds_comes_back_rendered() {
        let memory = MemoryThatAnswers(MemoryRecollection::of(vec![decision()]));

        let recalled = recall(&memory, &scope())
            .await
            .expect("something to recall");

        assert_eq!(recalled.scope().as_str(), "team:alpha");
        assert_eq!(recalled.entries().len(), 1);
    }

    #[tokio::test]
    async fn a_backend_that_declares_nothing_leaves_nothing_to_record() {
        let memory = MemoryThatAnswers(MemoryRecollection::Unsupported);

        assert!(recall(&memory, &scope()).await.is_none());
    }

    #[tokio::test]
    async fn a_scope_nobody_wrote_to_leaves_nothing_to_record() {
        let memory = MemoryThatAnswers(MemoryRecollection::nothing());

        assert!(recall(&memory, &scope()).await.is_none());
    }

    /// Memory is not the transaction: a backend that is out costs the
    /// session its recollection and not the session.
    #[tokio::test]
    async fn a_memory_that_cannot_be_read_answers_with_nothing_rather_than_an_error() {
        assert!(recall(&MemoryThatIsOut, &scope()).await.is_none());
    }
}
