//! [`SessionStream`] — a session as the fold of its stream.
//!
//! A ceremony is its event stream (ADR-012). Nothing here mutates a
//! session and stores it: a use case loads the fold, decides, and
//! appends the facts its decision produced, expecting the stream to be
//! at the version it decided against. A stream that moved in between
//! refuses the append, and the decision is made again against what is
//! actually there — or handed back as a conflict, as the command's
//! [`ConflictPolicy`] says.
//!
//! # Snapshots are a cache
//!
//! Loading is the latest snapshot plus the records after it. A
//! snapshot is written after every successful append and its failure
//! is logged, not returned: the facts landed, the session exists, and
//! the next load folds from wherever the last snapshot left off. A
//! store with no snapshots at all folds every stream from its opening
//! and is correct, only slower.
//!
//! # Correlation and causation are filled here
//!
//! Every fact of a stream correlates to its opening record, and each
//! is caused by the record that was the stream's head when it was
//! decided — within one batch, by the fact before it. A use case never
//! sets either: it does not know the head, and letting it guess would
//! be worse than leaving the fields empty.

use std::sync::Arc;

use made_core::entities::{AuditFact, CeremonyEvent, CeremonyInstance};
use made_core::error::DomainError;
use made_core::ports::{
    AppendOutcome, CeremonyEventStorePort, CeremonySnapshot, CeremonySnapshotStorePort,
};
use made_core::value_objects::{AuditActor, CeremonyId, StreamVersion};
use time::OffsetDateTime;

use super::{session_facts, ConflictPolicy, LoadedSession};

/// Loads the fold of a stream and appends what a decision produced.
pub struct SessionStream {
    events: Arc<dyn CeremonyEventStorePort>,
    snapshots: Arc<dyn CeremonySnapshotStorePort>,
}

impl std::fmt::Debug for SessionStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("SessionStream").finish()
    }
}

impl SessionStream {
    #[must_use]
    pub fn new(
        events: Arc<dyn CeremonyEventStorePort>,
        snapshots: Arc<dyn CeremonySnapshotStorePort>,
    ) -> Self {
        Self { events, snapshots }
    }

    /// Every stream the store holds, sorted by id.
    pub async fn ids(&self) -> Result<Vec<CeremonyId>, DomainError> {
        self.events.streams().await
    }

    /// The session at the stream's head: the latest snapshot, then
    /// every record after it, folded in order.
    ///
    /// The read starts one record before the snapshot's version rather
    /// than after it, so the record that was the head when the
    /// snapshot was taken comes back too — its id is what the next
    /// fact names as its cause, and this is the one read that learns
    /// it. A record with no event in it was sealed under schema
    /// version 1 and cannot be folded; the migration command (A7) is
    /// what turns such a journal into a stream.
    pub async fn load(&self, id: &CeremonyId) -> Result<LoadedSession, DomainError> {
        let (mut instance, mut version) = match self.snapshots.latest(id).await? {
            Some(snapshot) => (Some(snapshot.instance), snapshot.version),
            None => (None, StreamVersion::EMPTY),
        };
        let from = StreamVersion::new(version.value().saturating_sub(1));
        let mut head = None;
        for record in self.events.read(id, from).await? {
            let sequence = StreamVersion::from_sequence(record.sequence());
            let event = record.event().ok_or(DomainError::UnreadableCeremonyEvent {
                event_type: record.event_type().as_str(),
                version: record.schema_version(),
                reason: "the record carries no event to fold; \
                             a schema version 1 journal needs the migration command",
            })?;
            if sequence > version {
                match (&mut instance, event) {
                    (None, CeremonyEvent::CeremonyInstanceStarted(started)) => {
                        instance = Some(CeremonyInstance::from_started(started));
                    }
                    (None, _) => {
                        return Err(DomainError::InvariantViolated {
                            reason: "a ceremony stream opens with its start",
                        })
                    }
                    (Some(instance), event) => instance.apply(event),
                }
                version = sequence;
            }
            head = Some(record.event_id().clone());
        }
        let instance = instance.ok_or(DomainError::NotFound {
            what: "ceremony_instance",
        })?;
        Ok(LoadedSession::new(instance, version, head))
    }

    /// Open a stream with the events that start it.
    ///
    /// Appended against the empty version, which is what makes opening
    /// atomic: two starts of the same id both seal an opening, the
    /// store takes one, and the other is told the session exists —
    /// `AlreadyExists` rather than `Conflict`, because that is what
    /// happened and it is the answer a caller starting a session knows
    /// how to handle. The opening fact correlates to itself.
    ///
    /// A batch rather than one fact, because an opening is sometimes
    /// more than what was started: a session that recalled what earlier
    /// ones decided seals that beside it, in the same append, so a
    /// session cannot exist without what it was told. Everything after
    /// the first fact is correlated and chained exactly as
    /// [`Self::commit`] does.
    pub async fn open(
        &self,
        opening: Vec<CeremonyEvent>,
        actor: AuditActor,
        occurred_at: OffsetDateTime,
    ) -> Result<LoadedSession, DomainError> {
        let Some(CeremonyEvent::CeremonyInstanceStarted(started)) = opening.first() else {
            return Err(DomainError::InvariantViolated {
                reason: "a session opens with its start event",
            });
        };
        let mut instance = CeremonyInstance::from_started(started);
        let mut facts = session_facts::facts(&instance, opening, &actor, occurred_at)?;
        let mut correlation = None;
        let mut causation = None;
        for fact in &mut facts {
            fact.correlation_id = Some(correlation.get_or_insert(fact.event_id.clone()).clone());
            fact.causation_id = causation.take();
            causation = Some(fact.event_id.clone());
            instance.apply(&fact.event);
        }
        let head = causation;

        match self
            .events
            .append(instance.id(), StreamVersion::EMPTY, facts)
            .await?
        {
            AppendOutcome::Appended { version, .. } => {
                self.snapshot(&instance, version).await;
                Ok(LoadedSession::new(instance, version, head))
            }
            AppendOutcome::Conflict { .. } => Err(DomainError::AlreadyExists {
                what: "ceremony_instance",
            }),
        }
    }

    /// Append the facts a decision produced, expecting the stream to be
    /// where the session was loaded from.
    ///
    /// The facts' events are folded into the session on the way, so
    /// what comes back is the session at the new version and the
    /// caller never applies anything itself. A stream that moved is a
    /// conflict rather than an invariant violation: the caller lost a
    /// race and may decide again, which is a different instruction
    /// from "this can never work".
    pub async fn commit(
        &self,
        session: LoadedSession,
        facts: Vec<AuditFact>,
    ) -> Result<LoadedSession, DomainError> {
        let (mut instance, version, head) = session.into_parts();
        let correlation = session_facts::opening_event_id(instance.id())?;
        let mut causation = head;
        let facts = facts
            .into_iter()
            .map(|mut fact| {
                fact.correlation_id = Some(correlation.clone());
                fact.causation_id = causation.take();
                causation = Some(fact.event_id.clone());
                instance.apply(&fact.event);
                fact
            })
            .collect();

        match self.events.append(instance.id(), version, facts).await? {
            AppendOutcome::Appended { version, .. } => {
                self.snapshot(&instance, version).await;
                Ok(LoadedSession::new(instance, version, causation))
            }
            AppendOutcome::Conflict { .. } => Err(DomainError::Conflict {
                what: "ceremony_instance",
            }),
        }
    }

    /// Decide against the session and append, deciding again on a
    /// conflict as the policy allows.
    ///
    /// The first attempt is made against the session handed in, so a
    /// caller that loaded it to resolve the definition pays for no
    /// second read; every later attempt reloads the stream and hands
    /// the fresh fold to `decide`. That is the point of the closure:
    /// a decision made once and appended repeatedly would be the very
    /// stale write a conflict exists to refuse.
    pub async fn execute<F>(
        &self,
        session: LoadedSession,
        policy: ConflictPolicy,
        decide: F,
    ) -> Result<LoadedSession, DomainError>
    where
        F: Fn(&LoadedSession) -> Result<Vec<AuditFact>, DomainError>,
    {
        let id = session.instance.id().clone();
        let mut session = session;
        let mut attempts_left = policy.attempts();
        loop {
            attempts_left = attempts_left.saturating_sub(1);
            let facts = decide(&session)?;
            match self.commit(session, facts).await {
                Err(DomainError::Conflict { .. }) if attempts_left > 0 => {
                    session = self.load(&id).await?;
                }
                outcome => return outcome,
            }
        }
    }

    /// Cache the fold at this version. A failure is logged and
    /// swallowed: the append landed, and a missing snapshot costs the
    /// next load a longer fold, not a wrong one.
    async fn snapshot(&self, instance: &CeremonyInstance, version: StreamVersion) {
        let snapshot = CeremonySnapshot {
            version,
            instance: instance.clone(),
        };
        if let Err(error) = self.snapshots.save(snapshot).await {
            tracing::warn!(
                ceremony_id = %instance.id(),
                version = version.value(),
                %error,
                "the session snapshot was not written; the next load folds from the last one"
            );
        }
    }
}
