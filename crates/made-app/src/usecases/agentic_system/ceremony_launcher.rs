//! Starting one composed ceremony, the same way every time.
//!
//! Shared by instantiation and by advancing a run, because they are
//! the same act at different moments: a published definition is
//! started under a deterministic identity and its seats are filled
//! from the design's bindings. Two copies of this would be two chances
//! for a first round and a later one to be seated differently.

use std::collections::BTreeMap;
use std::sync::Arc;

use made_core::error::DomainError;
use made_core::value_objects::{
    AgenticSystemExecutionId, AuditActorId, AuditActorKind, CeremonyComposition, CeremonyContext,
    CeremonyId, LoopRound, ParticipantId, ParticipantMaterialization, RoleId, Specialty,
    SystemCeremonyId,
};

use crate::usecases::bind_ceremony_participants_input::BindCeremonyParticipantsInput;
use crate::usecases::start_ceremony_input::StartCeremonyInput;
use crate::usecases::{BindCeremonyParticipantsUseCase, StartPublishedCeremonyUseCase};

/// Opens instances for a run.
pub struct CeremonyLauncher {
    start: Arc<StartPublishedCeremonyUseCase>,
    seat: Arc<BindCeremonyParticipantsUseCase>,
}

impl std::fmt::Debug for CeremonyLauncher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("CeremonyLauncher").finish()
    }
}

impl CeremonyLauncher {
    #[must_use]
    pub const fn new(
        start: Arc<StartPublishedCeremonyUseCase>,
        seat: Arc<BindCeremonyParticipantsUseCase>,
    ) -> Self {
        Self { start, seat }
    }

    /// Start one composition and seat it.
    ///
    /// The instance identity is derived from the run, the composition
    /// and the round, so a retry opens the same instance rather than a
    /// second one — the stream refuses the second opening by itself,
    /// which is what makes the retry safe rather than merely likely.
    pub async fn launch(
        &self,
        execution: &AgenticSystemExecutionId,
        composition: &CeremonyComposition,
        round: LoopRound,
        context: CeremonyContext,
        materialized: &BTreeMap<ParticipantId, ParticipantMaterialization>,
        actor_id: &AuditActorId,
        actor_kind: AuditActorKind,
    ) -> Result<CeremonyId, DomainError> {
        let id = instance_id(execution, composition.id(), round)?;
        self.start
            .execute(StartCeremonyInput::new(
                id.clone(),
                composition.pin().name().clone(),
                composition.pin().version().clone(),
                context,
                actor_id.as_str(),
                actor_kind,
            ))
            .await?;
        let seating = seating(composition, materialized);
        if !seating.is_empty() {
            self.seat
                .execute(BindCeremonyParticipantsInput::new(
                    id.clone(),
                    seating,
                    actor_id.as_str(),
                    actor_kind,
                )?)
                .await?;
        }
        Ok(id)
    }
}

/// The identity one composition takes in one round of one run.
pub(super) fn instance_id(
    execution: &AgenticSystemExecutionId,
    ceremony: &SystemCeremonyId,
    round: LoopRound,
) -> Result<CeremonyId, DomainError> {
    CeremonyId::new(format!("{execution}-{ceremony}-{round}"))
}

/// Which specialty fills each seat, from what the host supplied.
///
/// Only bound participants appear. A seat whose participant is
/// unavailable is not seated with a placeholder; the composition it
/// belongs to is skipped before this is ever called.
fn seating(
    composition: &CeremonyComposition,
    materialized: &BTreeMap<ParticipantId, ParticipantMaterialization>,
) -> Vec<(RoleId, Specialty)> {
    composition
        .role_bindings()
        .iter()
        .filter_map(|(seat, participant)| {
            materialized
                .get(participant)
                .and_then(ParticipantMaterialization::specialty)
                .map(|specialty| (seat.clone(), specialty.clone()))
        })
        .collect()
}
