use time::OffsetDateTime;

use crate::entities::ceremony_events::CeremonyInstanceStarted;
use crate::entities::{
    CeremonyDefinition, CeremonyEvent, CeremonyInstance, PublishedCeremonyDefinition,
};
use crate::value_objects::{CeremonyContext, CeremonyDefinitionDigest, CeremonyId};

impl CeremonyInstance {
    /// The opening of a ceremony run from a definition supplied for it.
    ///
    /// A constructor rather than a command: there is no instance yet
    /// to decide against, and nothing to refuse — the definition was
    /// validated when it was built. The event carries everything
    /// [`Self::from_started`] needs to open the same instance without
    /// the definition in hand.
    #[must_use]
    pub fn decide_start(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
    ) -> CeremonyEvent {
        CeremonyEvent::CeremonyInstanceStarted(Self::opening(id, definition, context, now, None))
    }

    /// The opening of a ceremony bound to a published definition, its
    /// digest recorded so a later reader can check which one ran.
    #[must_use]
    pub fn decide_start_bound(
        id: CeremonyId,
        published: &PublishedCeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
    ) -> CeremonyEvent {
        CeremonyEvent::CeremonyInstanceStarted(Self::opening(
            id,
            published.definition(),
            context,
            now,
            Some(published.digest()),
        ))
    }

    /// What starting derives from the definition: the initial state
    /// and the steps that get a pending record.
    pub(in crate::entities::ceremony_instance) fn opening(
        id: CeremonyId,
        definition: &CeremonyDefinition,
        context: CeremonyContext,
        now: OffsetDateTime,
        bound_definition: Option<CeremonyDefinitionDigest>,
    ) -> CeremonyInstanceStarted {
        CeremonyInstanceStarted {
            ceremony_id: id,
            definition_name: definition.name().clone(),
            definition_version: definition.version().clone(),
            initial_state: definition.initial_state_id().clone(),
            step_ids: definition.steps().keys().cloned().collect(),
            context,
            bound_definition,
            created_at: now,
        }
    }
}
