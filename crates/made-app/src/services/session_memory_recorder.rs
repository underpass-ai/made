//! [`SessionMemoryRecorder`] — what a working session leaves behind.
//!
//! One place decides when something that happened becomes something a
//! later session can navigate. What it becomes is decided next door,
//! in the projection; keeping the two apart means the mapping can be
//! read and tested without a backend, and the timing can be read
//! without the mapping.
//!
//! # It is told, it does not ask
//!
//! The recorder is a subscriber of the stream
//! ([`CeremonyEventSubscriberPort`]), not something a use case calls.
//! Ten call sites deciding when a session had left something behind
//! was ten places to forget, and a use case that acquired a second
//! writer would have had to remember to call it too. What is
//! remembered is now a function of what was sealed, so a fact that
//! landed is a fact this saw.
//!
//! What a projection needs and a single record does not carry — the
//! ordinal of a response among its item\'s answers, whether an ending
//! is an ending — is folded from the stream the record belongs to,
//! never handed in by a caller.
//!
//! # Memory is not the transaction
//!
//! A session that cannot record what it decided still ran, and failing
//! it because a memory backend is unreachable would trade something
//! real for something recoverable. So nothing here returns an error to
//! its caller: a failed write is logged with what it would take to
//! make it again, and the session carries on.
//!
//! The cost is that memory can be lost quietly. Two things keep it
//! from being lost silently: every failure is logged at warning level
//! with its scope and key, and every key is derived from what is being
//! written about rather than from the clock — so making the same write
//! again is safe and lands once.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{
    CeremonyEventStorePort, CeremonyEventSubscriberPort, MemoryWriterPort, PositionedRecord,
};
use made_core::value_objects::{
    CeremonyId, CeremonyInterventionId, CeremonyRecordRef, MemoryProvenance, MemoryWrite,
    StreamVersion,
};

use super::memory_scope_resolver;
use super::session_memory_projection as projection;

/// Writes what a session decided, and why, into memory that outlives it.
pub struct SessionMemoryRecorder {
    memory: Arc<dyn MemoryWriterPort>,
    events: Arc<dyn CeremonyEventStorePort>,
}

impl std::fmt::Debug for SessionMemoryRecorder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SessionMemoryRecorder").finish()
    }
}

#[async_trait]
impl CeremonyEventSubscriberPort for SessionMemoryRecorder {
    /// Fold the stream these records continue, and remember what each
    /// of them turned out to be.
    ///
    /// The fold is up to and including each sealed record, so what a
    /// projection reads off the session is the session as that record
    /// left it — which is how a response finds its own ordinal and an
    /// ending finds the move that reached it.
    async fn observe(&self, records: &[PositionedRecord]) {
        let Some(first) = records.first() else {
            return;
        };
        let stream = first.record.ceremony_id().clone();
        let sealed: BTreeSet<u64> = records
            .iter()
            .map(|entry| entry.record.sequence().value())
            .collect();
        let history = match self.events.read(&stream, StreamVersion::EMPTY).await {
            Ok(history) => history,
            Err(error) => return Self::could_not_read(&stream, &error),
        };

        let mut session: Option<CeremonyInstance> = None;
        for record in &history {
            let Some(event) = record.event() else {
                continue;
            };
            match (&mut session, event) {
                (None, CeremonyEvent::CeremonyInstanceStarted(started)) => {
                    session = Some(CeremonyInstance::from_started(started));
                }
                (None, _) => continue,
                (Some(session), event) => session.apply(event),
            }
            let Some(session) = session.as_ref() else {
                continue;
            };
            if sealed.contains(&record.sequence().value()) {
                self.remember(session, event).await;
            }
        }
    }
}

impl SessionMemoryRecorder {
    #[must_use]
    pub fn new(memory: Arc<dyn MemoryWriterPort>, events: Arc<dyn CeremonyEventStorePort>) -> Self {
        Self { memory, events }
    }

    /// What one sealed event leaves behind, if anything.
    ///
    /// Four of the fourteen kinds are worth remembering; the rest are
    /// how a session got somewhere rather than what came of it.
    /// `EvidenceCollected` is deliberately not one of them: the pack
    /// it reports travels inside the response sealed with it, and that
    /// response is what becomes the contribution.
    async fn remember(&self, session: &CeremonyInstance, event: &CeremonyEvent) {
        match event {
            CeremonyEvent::InterventionResponded(responded) => {
                self.remember_contribution(session, &responded.intervention_id)
                    .await;
            }
            CeremonyEvent::HumanApprovalRecorded(recorded) => {
                let decided =
                    CeremonyRecordRef::guard_decision(recorded.approval.guard_name().clone());
                self.remember_guard_decision(session, &decided).await;
            }
            CeremonyEvent::HumanDeferralRecorded(recorded) => {
                let decided =
                    CeremonyRecordRef::guard_decision(recorded.deferral.guard_name().clone());
                self.remember_guard_decision(session, &decided).await;
            }
            // Numbered by its place among the reasons, which the fold
            // up to this record has just made last.
            CeremonyEvent::ReasonAsserted(_) => {
                self.remember_reason(session, session.reasons().len().saturating_sub(1))
                    .await;
            }
            CeremonyEvent::CeremonyCompleted(_) => self.remember_ending(session).await,
            _ => {}
        }
    }

    /// Remember the latest contribution to an agenda item.
    async fn remember_contribution(
        &self,
        instance: &CeremonyInstance,
        agenda_item: &CeremonyInterventionId,
    ) {
        let Some(ordinal) = instance
            .intervention(agenda_item)
            .map(|item| item.responses().len())
            .and_then(|count| u32::try_from(count.checked_sub(1)?).ok())
        else {
            return;
        };
        let record = CeremonyRecordRef::contribution(agenda_item.clone(), ordinal);
        match projection::contribution_entry(instance, &record) {
            Ok(Some(entry)) => {
                self.write(instance, MemoryWrite::unexplained(vec![entry]), &record)
                    .await;
            }
            Ok(None) => {}
            Err(error) => Self::could_not_project(instance, &record, &error),
        }
    }

    /// Remember a human decision on a guard.
    async fn remember_guard_decision(
        &self,
        instance: &CeremonyInstance,
        record: &CeremonyRecordRef,
    ) {
        match projection::guard_entry(instance, record) {
            Ok(Some(entry)) => {
                self.write(instance, MemoryWrite::unexplained(vec![entry]), record)
                    .await;
            }
            Ok(None) => {}
            Err(error) => Self::could_not_project(instance, record, &error),
        }
    }

    /// Remember how a session ended, once it has ended.
    ///
    /// Nothing is written for a session still running: an outcome is
    /// what came of the work, and a session in progress has not come
    /// of anything yet.
    async fn remember_ending(&self, instance: &CeremonyInstance) {
        match projection::ending_entry(instance) {
            Ok(Some((record, entry))) => {
                self.write(instance, MemoryWrite::unexplained(vec![entry]), &record)
                    .await;
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(
                ceremony_id = %instance.id(),
                %error,
                "an ending could not be turned into memory"
            ),
        }
    }

    /// Remember why one thing here led to another.
    ///
    /// Written on its own, because understanding usually arrives after
    /// the events it explains: both ends were remembered when they
    /// happened, and what is new is the edge.
    async fn remember_reason(&self, instance: &CeremonyInstance, ordinal: usize) {
        let Some(reason) = instance.reasons().get(ordinal) else {
            return;
        };
        let remembered = |record: &CeremonyRecordRef| Self::is_remembered(instance, record);
        match projection::relation(reason, &remembered) {
            Ok(Some(relation)) => {
                let key = format!("reason:{ordinal}");
                self.write_with_key(instance, MemoryWrite::reasons_only(vec![relation]), &key)
                    .await;
            }
            Ok(None) => tracing::debug!(
                ceremony_id = %instance.id(),
                "a reason was left out of memory because an end of it was not remembered"
            ),
            Err(error) => tracing::warn!(
                ceremony_id = %instance.id(),
                %error,
                "a reason could not be turned into memory"
            ),
        }
    }

    /// Whether a record is one this engine remembers at all.
    ///
    /// A step is machinery and an agenda item is a question; neither
    /// is one of the four things memory keeps. Asking here rather than
    /// letting the write fail is what keeps an edge from being sent
    /// into nothing.
    fn is_remembered(instance: &CeremonyInstance, record: &CeremonyRecordRef) -> bool {
        match record {
            CeremonyRecordRef::Contribution { .. } => {
                matches!(
                    projection::contribution_entry(instance, record),
                    Ok(Some(_))
                )
            }
            CeremonyRecordRef::GuardDecision { .. } => {
                matches!(projection::guard_entry(instance, record), Ok(Some(_)))
            }
            CeremonyRecordRef::Transition { ordinal } => {
                *ordinal as usize == instance.transitions().len()
                    && instance.completed_at().is_some()
            }
            CeremonyRecordRef::Step { .. } | CeremonyRecordRef::AgendaItem { .. } => false,
        }
    }

    async fn write(
        &self,
        instance: &CeremonyInstance,
        write: Result<MemoryWrite, DomainError>,
        record: &CeremonyRecordRef,
    ) {
        match projection::entry_id(record) {
            Ok(id) => self.write_with_key(instance, write, id.as_str()).await,
            Err(error) => Self::could_not_project(instance, record, &error),
        }
    }

    /// Write, and let nothing that goes wrong reach the session.
    async fn write_with_key(
        &self,
        instance: &CeremonyInstance,
        write: Result<MemoryWrite, DomainError>,
        discriminator: &str,
    ) {
        let write = match write {
            Ok(write) => write,
            Err(error) => {
                tracing::warn!(ceremony_id = %instance.id(), %error, "nothing to remember");
                return;
            }
        };
        let scope = match memory_scope_resolver::of_instance(instance) {
            Ok(scope) => scope,
            Err(error) => {
                tracing::warn!(ceremony_id = %instance.id(), %error, "a session has no memory scope");
                return;
            }
        };
        let key = match MemoryProvenance::new(instance.id().clone(), None, instance.updated_at())
            .idempotency_key(discriminator)
        {
            Ok(key) => key,
            Err(error) => {
                tracing::warn!(ceremony_id = %instance.id(), %error, "a memory write has no key");
                return;
            }
        };

        match self.memory.remember(&scope, write, &key).await {
            Ok(outcome) => tracing::debug!(
                scope = scope.as_str(),
                key,
                ?outcome,
                "a working session left something behind"
            ),
            Err(error) => tracing::warn!(
                scope = scope.as_str(),
                key,
                %error,
                "a working session could not record what it decided; the session is unaffected \
                 and the same write can be made again under this key"
            ),
        }
    }

    fn could_not_read(stream: &CeremonyId, error: &DomainError) {
        tracing::warn!(
            ceremony_id = %stream,
            %error,
            "a session\'s stream could not be read, so what it just decided was not remembered; \
             the same records can be projected again"
        );
    }

    fn could_not_project(
        instance: &CeremonyInstance,
        record: &CeremonyRecordRef,
        error: &DomainError,
    ) {
        tracing::warn!(
            ceremony_id = %instance.id(),
            ?record,
            %error,
            "something a session produced could not be turned into memory"
        );
    }
}
