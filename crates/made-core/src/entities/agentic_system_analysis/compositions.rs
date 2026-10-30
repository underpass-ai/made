//! Does each composition match the definition it pinned?
//!
//! Everything in this module compares the design against published
//! reality: the pin against what is published, the seating against the
//! seats the definition declares, the input wiring against the outputs
//! another definition promises, and a supervised guard against the
//! guard that exists.

use std::collections::BTreeMap;

use crate::entities::{AgenticSystemParts, PublishedProtocolDefinition};
use crate::value_objects::{
    AgenticSystemValidationFinding, AgenticSystemValidationLocus, GuardCondition, SystemProtocolId,
};

pub(super) fn collect(
    parts: AgenticSystemParts<'_>,
    resolved: &BTreeMap<SystemProtocolId, PublishedProtocolDefinition>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for (id, composition) in parts.protocols {
        let locus = AgenticSystemValidationLocus::protocol(id.clone());
        let Some(published) = resolved.get(id) else {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                format!(
                    "no published definition matches the pin `{}`",
                    composition.pin()
                ),
            ));
            continue;
        };
        if published.digest() != composition.pin().digest() {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                format!(
                    "the published `{}` has digest {} and the pin names {}",
                    composition.pin(),
                    published.digest(),
                    composition.pin().digest()
                ),
            ));
            continue;
        }
        seats(parts, id, published, findings);
        wiring(parts, id, resolved, findings);
    }
    approvals(parts, resolved, findings);
}

/// Every seat the definition declares is filled, and no seat it does
/// not declare is.
///
/// Both directions, because they fail differently: an unfilled seat
/// stalls the protocol at run time, and a binding to a seat that does
/// not exist is somebody's expectation that will silently never
/// happen.
fn seats(
    parts: AgenticSystemParts<'_>,
    id: &SystemProtocolId,
    published: &PublishedProtocolDefinition,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    let composition = &parts.protocols[id];
    for seat in published.definition().roles().keys() {
        if !composition.role_bindings().contains_key(seat) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::protocol_role(id.clone(), seat.clone()),
                format!("no participant is bound to seat `{seat}`"),
            ));
        }
    }
    for (seat, participant) in composition.role_bindings() {
        if !published.definition().roles().contains_key(seat) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::protocol_role(id.clone(), seat.clone()),
                format!(
                    "the pinned definition `{}` declares no seat `{seat}`",
                    composition.pin()
                ),
            ));
        }
        if !parts.participants.contains_key(participant) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::protocol_role(id.clone(), seat.clone()),
                format!("participant `{participant}` is not declared by this system"),
            ));
        }
    }
}

/// Every input taken from another protocol is an input this one
/// declares, taken from an output that one promises.
fn wiring(
    parts: AgenticSystemParts<'_>,
    id: &SystemProtocolId,
    resolved: &BTreeMap<SystemProtocolId, PublishedProtocolDefinition>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    let composition = &parts.protocols[id];
    let own = &resolved[id];
    for (input, source) in composition.inputs_from() {
        let locus = AgenticSystemValidationLocus::protocol_input(id.clone(), input.clone());
        if !own.definition().inputs().contains_key(input) {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus.clone(),
                format!(
                    "the pinned definition `{}` declares no input `{input}`",
                    composition.pin()
                ),
            ));
        }
        if !parts.protocols.contains_key(source.protocol()) {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                format!(
                    "protocol `{}` is not composed by this system",
                    source.protocol()
                ),
            ));
            continue;
        }
        let Some(producer) = resolved.get(source.protocol()) else {
            continue;
        };
        if !producer
            .definition()
            .outputs()
            .contains_key(source.output())
        {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                format!(
                    "protocol `{}` declares no output `{}`",
                    source.protocol(),
                    source.output()
                ),
            ));
        }
    }
}

/// A supervised approval points at a guard that exists and is one a
/// human answers.
///
/// Naming an automatic guard as a human approval would record a
/// decision nobody made, which is the one thing a human guard exists
/// to prevent.
fn approvals(
    parts: AgenticSystemParts<'_>,
    resolved: &BTreeMap<SystemProtocolId, PublishedProtocolDefinition>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for approval in parts.supervision.human_approvals() {
        let locus = AgenticSystemValidationLocus::guard(
            approval.protocol().clone(),
            approval.guard_name().clone(),
        );
        if !parts.protocols.contains_key(approval.protocol()) {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                format!(
                    "protocol `{}` is not composed by this system",
                    approval.protocol()
                ),
            ));
            continue;
        }
        let Some(published) = resolved.get(approval.protocol()) else {
            continue;
        };
        let Some(guard) = published.definition().guards().get(approval.guard_name()) else {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                format!(
                    "the pinned definition of `{}` declares no such guard",
                    approval.protocol()
                ),
            ));
            continue;
        };
        if !matches!(guard.condition(), GuardCondition::HumanApproval) {
            findings.push(AgenticSystemValidationFinding::refusal(
                locus,
                "this guard is not answered by a human, so requiring a human approval of it \
                 would record a decision nobody made",
            ));
        }
    }
}
