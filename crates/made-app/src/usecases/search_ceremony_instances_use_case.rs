use std::fmt;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::ports::CeremonyInstanceIndexPort;
use made_core::value_objects::{CeremonyId, CeremonyInstancePageLimit, TraceId};

use crate::services::SessionStream;
use crate::usecases::{
    CeremonyInstancePage, CeremonyInstanceRead, CeremonySearchCursorCodec,
    SearchCeremonyInstancesInput,
};

const MAX_INDEX_PAGES_SCANNED: usize = 10;

/// Pages authoritative ceremony folds without materializing every aggregate.
pub struct SearchCeremonyInstancesUseCase {
    index: Arc<dyn CeremonyInstanceIndexPort>,
    stream: Arc<SessionStream>,
    cursors: CeremonySearchCursorCodec,
}

impl fmt::Debug for SearchCeremonyInstancesUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchCeremonyInstancesUseCase")
            .finish()
    }
}

impl SearchCeremonyInstancesUseCase {
    #[must_use]
    pub fn new(
        index: Arc<dyn CeremonyInstanceIndexPort>,
        stream: Arc<SessionStream>,
        cursors: CeremonySearchCursorCodec,
    ) -> Self {
        Self {
            index,
            stream,
            cursors,
        }
    }

    #[tracing::instrument(name = "search_ceremony_instances", skip_all)]
    pub async fn execute(
        &self,
        input: &SearchCeremonyInstancesInput,
    ) -> Result<CeremonyInstancePage, DomainError> {
        let scan_limit = CeremonyInstancePageLimit::new(CeremonyInstancePageLimit::MAX)?;
        let mut instances = Vec::with_capacity(input.limit().value());
        let mut after = input
            .cursor()
            .map(|cursor| {
                self.cursors
                    .decode(cursor, input.id_prefix(), input.lifecycle())
            })
            .transpose()?;
        let mut more = false;

        'pages: for _ in 0..MAX_INDEX_PAGES_SCANNED {
            let page = self
                .index
                .ids_after(after.as_ref(), input.id_prefix(), scan_limit)
                .await?;
            if page.ids().is_empty() {
                more = false;
                break;
            }
            more = page.has_more();
            for (position, id) in page.ids().iter().enumerate() {
                after = Some(id.clone());
                let read = self.read(id).await?;
                let matches_lifecycle = input
                    .lifecycle()
                    .is_none_or(|phase| read.instance().lifecycle().phase() == phase);
                if matches_lifecycle {
                    instances.push(read);
                }
                if instances.len() == input.limit().value() {
                    more = position + 1 < page.ids().len() || page.has_more();
                    break 'pages;
                }
            }
            if !page.has_more() {
                more = false;
                break;
            }
        }

        let next_cursor = more
            .then(|| {
                after.as_ref().map(|after| {
                    self.cursors
                        .after(after, input.id_prefix(), input.lifecycle())
                })
            })
            .flatten();
        Ok(CeremonyInstancePage::new(instances, next_cursor))
    }

    async fn read(&self, id: &CeremonyId) -> Result<CeremonyInstanceRead, DomainError> {
        let instance = self.stream.load(id).await?.instance;
        let records = self.stream.records(id).await?;
        let head = records.last();
        let trace_id = head
            .and_then(|record| record.trace_id())
            .map(TraceId::new)
            .transpose()?;
        Ok(CeremonyInstanceRead::new(
            instance,
            trace_id,
            head.and_then(|record| record.correlation_id()).cloned(),
            head.and_then(|record| record.causation_id()).cloned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use made_core::entities::ceremony_events::{CeremonyCancelled, CeremonyPaused};
    use made_core::entities::{CeremonyEvent, CeremonyInstance};
    use made_core::ports::{CeremonyInstanceIdPage, CeremonyInstanceIndexPort};
    use made_core::value_objects::{
        CeremonyContext, CeremonyId, CeremonyIdPrefix, CeremonyLifecyclePhase, LifecycleReason,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{definition, now, stream, EventStoreFake};

    #[derive(Debug)]
    struct IndexFake {
        ids: Vec<CeremonyId>,
    }

    #[async_trait]
    impl CeremonyInstanceIndexPort for IndexFake {
        async fn ids_after(
            &self,
            after: Option<&CeremonyId>,
            id_prefix: Option<&CeremonyIdPrefix>,
            limit: CeremonyInstancePageLimit,
        ) -> Result<CeremonyInstanceIdPage, DomainError> {
            let mut ids: Vec<_> = self
                .ids
                .iter()
                .filter(|id| after.is_none_or(|after| *id > after))
                .filter(|id| {
                    id_prefix.is_none_or(|prefix| id.as_str().starts_with(prefix.as_str()))
                })
                .take(limit.value() + 1)
                .cloned()
                .collect();
            let has_more = ids.len() > limit.value();
            ids.truncate(limit.value());
            Ok(CeremonyInstanceIdPage::new(ids, has_more))
        }
    }

    #[tokio::test]
    async fn lifecycle_filter_folds_authoritative_streams_and_cursor_marks_last_examined_id() {
        let definition = definition();
        let store = Arc::new(EventStoreFake::default());
        let mut ids = Vec::new();
        for (id, event) in [
            ("a-running", None),
            (
                "b-paused",
                Some(CeremonyEvent::CeremonyPaused(CeremonyPaused {
                    reason: LifecycleReason::new("hold").unwrap(),
                    paused_at: now(),
                })),
            ),
            (
                "c-ended",
                Some(CeremonyEvent::CeremonyCancelled(CeremonyCancelled {
                    reason: LifecycleReason::new("stop").unwrap(),
                    cancelled_at: now(),
                })),
            ),
        ] {
            let id = CeremonyId::new(id).unwrap();
            let mut instance =
                CeremonyInstance::start(id.clone(), &definition, CeremonyContext::empty(), now())
                    .unwrap();
            if let Some(event) = event {
                instance.apply(&event);
            }
            store.save(&instance).await.unwrap();
            ids.push(id);
        }
        let cursors = CeremonySearchCursorCodec::new(
            crate::usecases::CeremonySearchCursorKey::new([9; 32]),
            crate::usecases::CeremonySearchCursorNamespace::new("test-store", "test-policy")
                .unwrap(),
        );
        let usecase = SearchCeremonyInstancesUseCase::new(
            Arc::new(IndexFake { ids }),
            stream(store),
            cursors.clone(),
        );
        let input = SearchCeremonyInstancesInput::new(
            None,
            CeremonyInstancePageLimit::new(1).unwrap(),
            None,
            Some(CeremonyLifecyclePhase::Paused),
        );

        let first = usecase.execute(&input).await.unwrap();
        assert_eq!(first.reads()[0].instance().id().as_str(), "b-paused");
        let next_cursor = first.next_cursor().unwrap();
        assert_eq!(
            cursors
                .decode(next_cursor, None, Some(CeremonyLifecyclePhase::Paused))
                .unwrap()
                .as_str(),
            "b-paused"
        );

        let rest = usecase
            .execute(&SearchCeremonyInstancesInput::new(
                Some(next_cursor.clone()),
                CeremonyInstancePageLimit::new(1).unwrap(),
                None,
                Some(CeremonyLifecyclePhase::Paused),
            ))
            .await
            .unwrap();
        assert!(rest.reads().is_empty());
        assert!(rest.next_cursor().is_none());
    }
}
