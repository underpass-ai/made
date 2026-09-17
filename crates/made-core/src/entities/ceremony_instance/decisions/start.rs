use time::OffsetDateTime;

use crate::entities::ceremony_events::{CeremonyInstanceStarted, MemoryRecalled};
use crate::entities::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, PublishedCeremonyDefinition,
};
use crate::error::DomainError;
use crate::value_objects::{
    CeremonyContext, CeremonyDefinitionDigest, CeremonyId, SessionRecollection,
};

impl CeremonyInstance {
    /// The opening of a ceremony run from a definition supplied for it.
    ///
    /// A constructor rather than a command: there is no instance yet
    /// to decide against. Required inputs are checked against this run's
    /// context before any opening event is built. The event carries everything
    /// [`Self::from_started`] needs to open the same instance without
    /// the definition in hand.
    ///
    /// A batch rather than one event, because an opening is sometimes
    /// two facts: what was started, and what it was told. See
    /// [`Self::opening_batch`].
    pub fn decide_start(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(id, definition, context, now, None)?,
            recollection,
            now,
        ))
    }

    /// The opening of a ceremony bound to a published definition, its
    /// digest recorded so a later reader can check which one ran.
    pub fn decide_start_bound(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        Ok(Self::opening_batch(
            Self::opening(
                id,
                published.definition(),
                context,
                now,
                Some(published.digest()),
            )?,
            recollection,
            now,
        ))
    }

    /// The opening, and the recollection when there was one.
    ///
    /// The recollection is decided outside — reading memory is IO, and
    /// `decide` stays pure — and handed in. What is decided here is
    /// whether it becomes a fact: a recollection that came back empty
    /// appends nothing, so a session with nothing to recall, which is
    /// every session that declares no scope, has exactly the stream it
    /// had before memory could be read at all.
    fn opening_batch(
        opening: CeremonyInstanceStarted,
        recollection: Option<SessionRecollection>,
        now: OffsetDateTime,
    ) -> Vec<CeremonyEvent> {
        let mut events = vec![CeremonyEvent::CeremonyInstanceStarted(opening)];
        if let Some(recollection) = recollection.filter(|recalled| !recalled.is_empty()) {
            events.push(CeremonyEvent::MemoryRecalled(MemoryRecalled {
                recollection,
                recalled_at: now,
            }));
        }
        events
    }

    /// What starting derives from the definition: the initial state
    /// and the steps that get a pending record.
    pub(in crate::entities::ceremony_instance) fn opening(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
        bound_definition: Option<CeremonyDefinitionDigest>,
    ) -> Result<CeremonyInstanceStarted, DomainError> {
        let missing = definition
            .inputs()
            .values()
            .filter(|input| input.requirement().is_required())
            .filter(|input| context.attributes().get(input.name().as_str()).is_none())
            .map(|input| input.name().as_str())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(DomainError::InvalidDocument {
                reason: format!("missing required ceremony inputs: {}", missing.join(", ")),
            });
        }
        Ok(CeremonyInstanceStarted {
            ceremony_id: id,
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            initial_state: definition.initial_state_id().clone(),
            step_ids: definition.steps().keys().cloned().collect(),
            context,
            bound_definition,
            created_at: now,
        })
    }
}
