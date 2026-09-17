use std::fmt;
use std::sync::Arc;

use made_core::entities::ceremony_events::InstanceImported;
use made_core::entities::{CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventStorePort, LegacyCeremonySnapshot, LegacyCeremonySnapshotSourcePort,
};
use made_core::value_objects::{AuditActor, CeremonyId, StreamVersion};

use crate::services::session_facts;

use super::{PreStreamImport, ReadWholeCeremonyEventsUseCase};

/// Opens a stream for every session a pre-stream store still holds
/// (ADR-012).
///
/// One genesis event per session, appended against an empty stream, and
/// then read back and folded: the import only counts a session as
/// imported once the stream it wrote folds back to the snapshot it was
/// given. That check is the whole point — the caller installs the
/// migrated file only when every session holds, so a store that would
/// have come back different is never the one an operator is left with.
///
/// It appends straight through the event store rather than through a
/// session: there is no session yet, and there is no command that
/// produces this event. `decide` never emits an import, because an
/// import is not something a ceremony does.
pub struct ImportPreStreamInstancesUseCase {
    legacy: Arc<dyn LegacyCeremonySnapshotSourcePort>,
    events: Arc<dyn CeremonyEventStorePort>,
}

impl fmt::Debug for ImportPreStreamInstancesUseCase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportPreStreamInstancesUseCase")
            .finish()
    }
}

impl ImportPreStreamInstancesUseCase {
    #[must_use]
    pub fn new(
        legacy: Arc<dyn LegacyCeremonySnapshotSourcePort>,
        events: Arc<dyn CeremonyEventStorePort>,
    ) -> Self {
        Self { legacy, events }
    }

    /// Import every session without a stream, or fail having imported
    /// none of them that did not hold.
    ///
    /// `imported_at` is passed rather than read from a clock: one run
    /// is one migration, and every session it brings forward is stamped
    /// with the same instant.
    #[tracing::instrument(name = "import_pre_stream_instances", skip_all)]
    pub async fn execute(
        &self,
        actor: AuditActor,
        imported_at: time::OffsetDateTime,
    ) -> Result<PreStreamImport, DomainError> {
        let mut imported = Vec::new();
        for legacy in self.legacy.instances_without_a_stream().await? {
            imported.push(self.import(&legacy, actor.clone(), imported_at).await?);
        }
        Ok(PreStreamImport::new(imported))
    }

    async fn import(
        &self,
        legacy: &LegacyCeremonySnapshot,
        actor: AuditActor,
        imported_at: time::OffsetDateTime,
    ) -> Result<CeremonyId, DomainError> {
        let ceremony_id = legacy.instance.id().clone();
        let event = CeremonyEvent::InstanceImported(InstanceImported {
            ceremony_id: ceremony_id.clone(),
            definition_name: legacy.instance.definition_name().clone(),
            definition_version: legacy.instance.definition_version().clone(),
            snapshot: Box::new(legacy.instance.clone()),
            legacy_journal_head_hash: legacy.journal_head_hash,
            legacy_revision: legacy.revision,
            imported_at,
        });
        let fact = session_facts::fact(&legacy.instance, event, actor, imported_at)?;

        // The expectation is emptiness itself: a session that grew a
        // stream between the listing and here is one somebody else is
        // already driving, and writing a genesis event under it would
        // make its history start twice.
        let outcome = self
            .events
            .append(&ceremony_id, StreamVersion::EMPTY, vec![fact])
            .await?;
        if outcome.is_conflict() {
            return Err(DomainError::Conflict {
                what: "ceremony_stream",
            });
        }

        self.verify_folds_back(&ceremony_id, &legacy.instance)
            .await?;
        Ok(ceremony_id)
    }

    /// Read what was written and fold it: the imported session has to
    /// come back out of the store it went into.
    ///
    /// Read back rather than folded from what was sent, because what
    /// the store kept is what the next reader gets, and a store that
    /// lost a field would otherwise pass its own migration.
    async fn verify_folds_back(
        &self,
        ceremony_id: &CeremonyId,
        expected: &CeremonyInstance,
    ) -> Result<(), DomainError> {
        let records = ReadWholeCeremonyEventsUseCase::new(self.events.clone())
            .execute(ceremony_id)
            .await?;
        let events = records
            .iter()
            .map(|record| {
                record.event().ok_or(DomainError::InvariantViolated {
                    reason: "an imported stream holds a record with no event",
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if &CeremonyInstance::rehydrate(events)? != expected {
            return Err(DomainError::InvariantViolated {
                reason: "an imported stream does not fold back to the session it imported",
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use made_core::value_objects::{
        AuditActorKind, AuditEventType, AuditRecordHash, CeremonyContext, CeremonyRevision, StateId,
    };

    use super::*;
    use crate::usecases::ceremony_test_support::{
        definition, now, started_instance, EventStoreFake,
    };

    /// A pre-stream store, as the import sees it.
    #[derive(Debug, Default)]
    struct LegacySourceFake {
        snapshots: Vec<LegacyCeremonySnapshot>,
    }

    #[async_trait]
    impl LegacyCeremonySnapshotSourcePort for LegacySourceFake {
        async fn instances_without_a_stream(
            &self,
        ) -> Result<Vec<LegacyCeremonySnapshot>, DomainError> {
            Ok(self.snapshots.clone())
        }
    }

    fn actor() -> AuditActor {
        AuditActor::new("migrate-store", AuditActorKind::Engine, None).unwrap()
    }

    fn legacy(instance: CeremonyInstance) -> LegacyCeremonySnapshot {
        LegacyCeremonySnapshot {
            instance,
            revision: CeremonyRevision::INITIAL,
            journal_head_hash: Some(AuditRecordHash::from_bytes([4_u8; 32])),
        }
    }

    /// A session under its own id, so two of them can be imported in
    /// one run without colliding.
    fn opened(id: &str) -> CeremonyInstance {
        CeremonyInstance::start(
            CeremonyId::new(id).unwrap(),
            &definition(),
            CeremonyContext::empty(),
            now(),
        )
    }

    /// A session the old engine had already moved: the fold has to
    /// reproduce the state, not the opening.
    fn moved_on() -> CeremonyInstance {
        let mut instance = opened("moved-on");
        instance.apply(&CeremonyEvent::CeremonyCompleted(
            made_core::entities::ceremony_events::CeremonyCompleted {
                final_state: StateId::new("DONE").unwrap(),
                completed_at: now(),
            },
        ));
        instance
    }

    fn use_case(
        snapshots: Vec<LegacyCeremonySnapshot>,
        events: Arc<EventStoreFake>,
    ) -> ImportPreStreamInstancesUseCase {
        ImportPreStreamInstancesUseCase::new(Arc::new(LegacySourceFake { snapshots }), events)
    }

    #[tokio::test]
    async fn an_empty_legacy_table_imports_nothing() {
        let events = Arc::new(EventStoreFake::default());

        let report = use_case(Vec::new(), events.clone())
            .execute(actor(), now())
            .await
            .unwrap();

        assert!(report.is_empty());
        assert_eq!(report.len(), 0);
        assert!(events.facts().await.is_empty());
    }

    #[tokio::test]
    async fn an_imported_session_folds_back_to_the_snapshot_it_carried() {
        let events = Arc::new(EventStoreFake::default());
        let snapshot = moved_on();

        let report = use_case(vec![legacy(snapshot.clone())], events.clone())
            .execute(actor(), now())
            .await
            .unwrap();

        assert_eq!(report.imported(), [snapshot.id().clone()]);
        assert_eq!(events.saved(snapshot.id()).await, snapshot);
    }

    #[tokio::test]
    async fn the_genesis_record_carries_the_legacy_head_and_revision() {
        let events = Arc::new(EventStoreFake::default());
        let snapshot = moved_on();

        use_case(vec![legacy(snapshot.clone())], events.clone())
            .execute(actor(), now())
            .await
            .unwrap();

        let facts = events.facts().await;
        assert_eq!(facts.len(), 1);
        assert_eq!(
            facts[0].event.event_type(),
            AuditEventType::InstanceImported
        );
        let CeremonyEvent::InstanceImported(imported) = &facts[0].event else {
            panic!("the genesis fact is the import");
        };
        assert_eq!(
            imported.legacy_journal_head_hash,
            Some(AuditRecordHash::from_bytes([4_u8; 32]))
        );
        assert_eq!(imported.legacy_revision, CeremonyRevision::INITIAL);
        assert_eq!(imported.imported_at, now());
        assert_eq!(imported.snapshot.as_ref(), &snapshot);
    }

    #[tokio::test]
    async fn every_session_of_the_legacy_table_is_imported() {
        let events = Arc::new(EventStoreFake::default());
        let first = opened("still-open");
        let second = moved_on();

        let report = use_case(
            vec![legacy(first.clone()), legacy(second.clone())],
            events.clone(),
        )
        .execute(actor(), now())
        .await
        .unwrap();

        assert_eq!(report.len(), 2);
        assert_eq!(events.saved(first.id()).await, first);
        assert_eq!(events.saved(second.id()).await, second);
    }

    /// A session that grew a stream between the listing and the append
    /// is somebody else's now; the import refuses rather than opening
    /// its history a second time.
    #[tokio::test]
    async fn a_session_that_already_has_a_stream_is_a_conflict() {
        let events = Arc::new(EventStoreFake::default());
        let instance = started_instance(&definition());
        events.save(&instance).await.unwrap();

        let error = use_case(vec![legacy(instance)], events)
            .execute(actor(), now())
            .await
            .unwrap_err();

        assert!(
            matches!(
                error,
                DomainError::Conflict {
                    what: "ceremony_stream"
                }
            ),
            "expected a stream conflict, got {error:?}"
        );
    }
}
