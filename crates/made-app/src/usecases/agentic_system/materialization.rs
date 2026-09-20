//! Who a run could actually find, and who it could not.
//!
//! One rule, applied the same way for every participant: the design
//! says what is needed, the host says what it has, and anything the
//! host did not offer is recorded as unavailable with the reason.
//! Nothing stands in for a missing party — a design proved by a
//! stand-in has been proved about nothing.

use std::collections::{BTreeMap, BTreeSet};

use made_core::entities::AgenticSystem;
use made_core::error::DomainError;
use made_core::value_objects::{
    Capability, ParticipantId, ParticipantMaterialization, SystemCeremonyId, UnavailabilityReason,
};

use super::ParticipantOffer;

/// Decide, for every logical participant, whether the host supplied
/// somebody who can do what the design asks.
pub(super) fn of(
    system: &AgenticSystem,
    offers: &BTreeMap<ParticipantId, ParticipantOffer>,
) -> Result<BTreeMap<ParticipantId, ParticipantMaterialization>, DomainError> {
    let mut materialized = BTreeMap::new();
    for (id, participant) in system.participants() {
        let needed = needed_capabilities(system, id);
        let materialization = match offers.get(id) {
            None => ParticipantMaterialization::unavailable(UnavailabilityReason::new(format!(
                "the host offered nobody for participant `{id}`"
            ))?),
            Some(offer) => match offer.missing(&needed) {
                Some(capability) => {
                    ParticipantMaterialization::unavailable(UnavailabilityReason::new(format!(
                        "`{}` was offered for participant `{id}` but does not supply capability \
                         `{capability}`",
                        offer.specialty()
                    ))?)
                }
                None => ParticipantMaterialization::bound(offer.specialty().clone()),
            },
        };
        let _ = participant;
        materialized.insert(id.clone(), materialization);
    }
    Ok(materialized)
}

/// Why one composition cannot run, if it cannot.
///
/// A ceremony needs everybody seated at it. One missing participant is
/// enough, and the reason names them, because "skipped" on its own is
/// not something anybody can act on.
pub(super) fn blocking(
    system: &AgenticSystem,
    materialized: &BTreeMap<ParticipantId, ParticipantMaterialization>,
    ceremony: &SystemCeremonyId,
) -> Result<Option<UnavailabilityReason>, DomainError> {
    let Some(composition) = system.ceremonies().get(ceremony) else {
        return Ok(None);
    };
    for participant in composition.role_bindings().values() {
        let unavailable = materialized
            .get(participant)
            .and_then(ParticipantMaterialization::reason);
        if let Some(reason) = unavailable {
            return Ok(Some(UnavailabilityReason::new(format!(
                "ceremony `{ceremony}` needs participant `{participant}`: {reason}"
            ))?));
        }
        if !materialized.contains_key(participant) {
            return Ok(Some(UnavailabilityReason::new(format!(
                "ceremony `{ceremony}` is bound to participant `{participant}`, which the design \
                 does not declare"
            ))?));
        }
    }
    Ok(None)
}

/// Everything whoever plays this participant has to be able to do:
/// what the participant declares, plus what its role's requested
/// profile asks for.
fn needed_capabilities(system: &AgenticSystem, id: &ParticipantId) -> BTreeSet<Capability> {
    let Some(participant) = system.participants().get(id) else {
        return BTreeSet::new();
    };
    let mut needed: BTreeSet<Capability> = participant.binding().capabilities().clone();
    if let Some(profile) = system.profiles().get(participant.role()) {
        needed.extend(profile.required_capabilities().iter().cloned());
    }
    needed
}
