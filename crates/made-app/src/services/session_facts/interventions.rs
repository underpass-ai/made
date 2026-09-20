//! What an intervention event is *about*.
//!
//! Its own file because the identity of a delivery acknowledgement has
//! a rule of its own — it must distinguish every legitimate repeat and
//! still fit inside an event id — and a rule with a reason belongs
//! where the reason can be read.

use made_core::entities::CeremonyEvent;

/// Sixteen hex characters of SHA-256: enough that two destinations do
/// not collide, short enough that an identifier built from it stays
/// inside the bounds an event id has.
fn short_digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    digest.update(value.as_bytes());
    format!("{:x}", digest.finalize())[..16].to_owned()
}

pub(super) fn intervention_about(event: &CeremonyEvent) -> String {
    match event {
        CeremonyEvent::InterventionRequested(requested) => {
            format!("intervention:{}", requested.intervention.id())
        }
        CeremonyEvent::InterventionResponded(responded) => format!(
            "intervention:{}:{}",
            responded.intervention_id,
            responded.response.role_id()
        ),
        CeremonyEvent::InterventionClosed(closed) => {
            format!("intervention:{}", closed.intervention_id)
        }
        // The delivery identity, digested rather than spelled out.
        //
        // It already contains the ceremony, the item and the whole
        // destination including the process generation, so it
        // distinguishes every legitimate repeat — and it is built from
        // three separately bounded strings, so spelled out it can
        // exceed what an event id may be. The item is kept readable
        // because that is what somebody scanning the stream is looking
        // for; which offer it was is in the payload.
        CeremonyEvent::InterventionDeliveryAcknowledged(acknowledged) => format!(
            "intervention:{}:delivery:{}",
            acknowledged.intervention_id,
            short_digest(acknowledged.ack.delivery_id().as_str())
        ),
        CeremonyEvent::EvidenceCollected(collected) => format!(
            "intervention:{}:source:{}",
            collected.intervention_id, collected.source_id
        ),
        _ => unreachable!("caller filters intervention events"),
    }
}
