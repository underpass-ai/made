//! Live ceremony state: application view → proto.
//!
//! Renders the same [`CeremonyInstanceView`] the embedded adapter
//! renders to JSON. Neither side derives anything of its own, which is
//! what makes "the same working session over either transport" a
//! property of the code rather than a promise in a document.

use made_app::usecases::{CeremonyInstanceView, CeremonyTransitionView};
use made_core::entities::{CeremonyInstance, CeremonyIntervention};
use made_core::value_objects::{
    CeremonyDefinitionDigest, CeremonyGuardDeferral, CeremonyId, CeremonyInterventionResponse,
    CeremonyLifecycle, CeremonyParticipantBinding, CeremonyReason, CeremonyRecordRef,
    RecalledEntry, RoleId, SessionRecollection, StepDeadline, StepId,
};
use made_proto::v1 as pb;
use time::OffsetDateTime;

use super::attributes::attributes_to_struct;
use super::budget::budget_account_id_to_proto;
use super::ceremony_instance_children::{child_group_state_from, lineage_state_from};
use super::ceremony_instance_step::step_state_from;

/// One instant, one rendering.
///
/// Four of the timestamps this view carries used to be
/// `OffsetDateTime`'s `Display` — not an interchange format, and not
/// what the embedded adapter renders from the same view, so the same
/// moment read two ways depending on which engine served the call.
/// RFC 3339 on every one of them, as `bound_at` already was.
pub(super) fn moment(at: OffsetDateTime) -> String {
    at.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

pub fn ceremony_instance_state_from(view: &CeremonyInstanceView<'_>) -> pb::CeremonyInstanceState {
    let instance = view.instance();
    let lifecycle = instance.lifecycle();

    pb::CeremonyInstanceState {
        ceremony_id: instance.id().as_str().to_owned(),
        definition_name: instance.definition_name().as_str().to_owned(),
        definition_version: instance.definition_version().as_str().to_owned(),
        // Empty rather than absent: proto3 has no null, and an instance
        // started from a definition supplied for the run must not look
        // like one bound to a published version.
        bound_definition_digest: instance
            .bound_definition()
            .map(CeremonyDefinitionDigest::to_hex)
            .unwrap_or_default(),
        current_state: instance.current_state().as_str().to_owned(),
        completed: view.is_completed(),
        next_step_id: view
            .next_step_id()
            .map(StepId::as_str)
            .unwrap_or_default()
            .to_owned(),
        waiting_for_human: view
            .waiting_for_human()
            .iter()
            .map(|name| name.as_str().to_owned())
            .collect(),
        guard_deferrals: instance
            .guard_deferrals()
            .iter()
            .map(guard_deferral_state_from)
            .collect(),
        transitions: view
            .transitions()
            .iter()
            .map(transition_state_from)
            .collect(),
        steps: view.steps().iter().map(step_state_from).collect(),
        interventions: instance
            .interventions()
            .iter()
            .map(intervention_state_from)
            .collect(),
        open_intervention_ids: open_intervention_ids(instance),
        context: Some(attributes_to_struct(instance.context().attributes())),
        participant_bindings: view
            .participant_bindings()
            .values()
            .map(participant_binding_state_from)
            .collect(),
        reasons: view.reasons().iter().map(reason_state_from).collect(),
        // It was projected, so it is readable. The one caller that can
        // say otherwise is the listing below.
        rehydratable: true,
        unrehydratable_reason: String::new(),
        recollection: instance.recollection().map(recollection_state_from),
        trace_id: String::new(),
        correlation_id: String::new(),
        causation_id: String::new(),
        claimable_step_ids: view
            .claimable_step_ids()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        current_state_visit: instance.current_state_visit().get(),
        current_state_iteration: instance.current_state_iteration().get(),
        state_repeat_max_iterations: view
            .definition()
            .state(instance.current_state())
            .and_then(|state| state.repeat_policy())
            .map_or(0, |policy| policy.max_iterations().get()),
        state_repeat_condition_satisfied: instance
            .state_repeat_condition_is_satisfied(view.definition()),
        state_repeat_limit_reached: instance.state_repeat_limit_reached(view.definition()),
        lineage: instance.lineage().map(lineage_state_from),
        child_groups: instance
            .child_groups()
            .values()
            .map(child_group_state_from)
            .collect(),
        lifecycle: lifecycle.phase().as_label().to_owned(),
        end_reason: lifecycle
            .end_reason()
            .map(|reason| reason.as_label().to_owned())
            .unwrap_or_default(),
        paused_at: lifecycle_changed_at(&lifecycle, lifecycle.is_paused()),
        ended_at: lifecycle_changed_at(&lifecycle, lifecycle.is_ended()),
        ceremony_deadline_at: instance
            .ceremony_deadline()
            .map(|deadline| moment(deadline.at()))
            .unwrap_or_default(),
        state_deadline_at: instance
            .state_deadline()
            .map(|deadline| moment(deadline.at()))
            .unwrap_or_default(),
        step_deadlines: instance
            .step_deadlines()
            .values()
            .map(step_deadline_state_from)
            .collect(),
        budget_account_id: budget_account_id_to_proto(instance.budget_account_id()),
        succession: view
            .succession()
            .map(super::ceremony_succession::succession_to_proto),
        successor_plan: view
            .successor_plan()
            .map(super::ceremony_succession::plan_to_proto),
    }
}

fn lifecycle_changed_at(lifecycle: &CeremonyLifecycle, active: bool) -> String {
    active
        .then(|| lifecycle.changed_at().map(moment))
        .flatten()
        .unwrap_or_default()
}

fn step_deadline_state_from(deadline: &StepDeadline) -> pb::CeremonyStepDeadlineState {
    pb::CeremonyStepDeadlineState {
        step_id: deadline.step_id().as_str().to_owned(),
        state_visit: deadline.state_visit().get(),
        state_iteration: deadline.state_iteration().get(),
        step_iteration: deadline.step_iteration().get(),
        attempt: deadline.attempt().get(),
        claim_fence: deadline.claim_fence().as_str().to_owned(),
        deadline_at: moment(deadline.at()),
        finished_by_role_id: deadline.finished_by().as_str().to_owned(),
    }
}

/// What this session was told when it opened.
///
/// Absent rather than empty for a session that was told nothing: proto
/// message presence is the one place in this contract where absence can
/// be said, and "recalled nothing" is not "recalled an empty scope".
pub(super) fn recollection_state_from(
    recollection: &SessionRecollection,
) -> pb::CeremonyRecollectionState {
    pb::CeremonyRecollectionState {
        scope: recollection.scope().as_str().to_owned(),
        entries: recollection
            .entries()
            .iter()
            .map(recalled_entry_state_from)
            .collect(),
        truncated: recollection.completeness().is_truncated(),
    }
}

fn recalled_entry_state_from(entry: &RecalledEntry) -> pb::CeremonyRecalledEntryState {
    pb::CeremonyRecalledEntryState {
        entry_id: entry.id().as_str().to_owned(),
        kind: entry.kind().as_label().to_owned(),
        summary: entry.summary().to_owned(),
        from_ceremony_id: entry.from_ceremony().as_str().to_owned(),
        observed_at: moment(entry.observed_at()),
    }
}

/// A listing entry for a session whose definition this store does not
/// hold.
///
/// The stream is there and the id is all that can be rendered from it,
/// so the listing says so for that one entry instead of failing the
/// whole call. This is the same answer the in-process backend gives,
/// which is the point of it existing here.
pub fn unrehydratable_ceremony_instance_state_from(
    ceremony_id: &CeremonyId,
    reason: &str,
) -> pb::CeremonyInstanceState {
    pb::CeremonyInstanceState {
        ceremony_id: ceremony_id.as_str().to_owned(),
        rehydratable: false,
        unrehydratable_reason: reason.to_owned(),
        ..pb::CeremonyInstanceState::default()
    }
}

/// A reason, on its way back out.
///
/// The edges were write-only over the wire until now: a seat could
/// explain a session and nobody holding that session could find the
/// explanation again.
fn reason_state_from(reason: &CeremonyReason) -> pb::CeremonyReasonState {
    pb::CeremonyReasonState {
        from: Some(record_ref_state_from(reason.from())),
        to: Some(record_ref_state_from(reason.to())),
        kind: reason.kind().as_label().to_owned(),
        why: reason.why().to_owned(),
        confidence: reason.confidence().as_label().to_owned(),
        asserted_by_role_id: reason
            .asserted_by()
            .map(|role_id| role_id.as_str().to_owned())
            .unwrap_or_default(),
        asserted_at: moment(reason.asserted_at()),
    }
}

fn participant_binding_state_from(
    binding: &CeremonyParticipantBinding,
) -> pb::CeremonyParticipantBindingState {
    pb::CeremonyParticipantBindingState {
        role_id: binding.role_id().as_str().to_owned(),
        specialty: binding.specialty().as_str().to_owned(),
        bound_at: moment(binding.bound_at()),
    }
}

fn transition_state_from(
    transition: &CeremonyTransitionView<'_>,
) -> pb::CeremonyAvailableTransition {
    pb::CeremonyAvailableTransition {
        trigger: transition.transition().trigger().as_str().to_owned(),
        to: transition.transition().to().as_str().to_owned(),
        enabled: transition.is_enabled(),
        guards: transition
            .guards()
            .iter()
            .map(|guard| pb::CeremonyTransitionGuard {
                name: guard.name().as_str().to_owned(),
                kind: if guard.is_human() {
                    "human"
                } else {
                    "automated"
                }
                .to_owned(),
                satisfied: guard.is_satisfied(),
            })
            .collect(),
    }
}

fn guard_deferral_state_from(deferral: &CeremonyGuardDeferral) -> pb::CeremonyGuardDeferralState {
    pb::CeremonyGuardDeferralState {
        guard_name: deferral.guard_name().as_str().to_owned(),
        statement: deferral.content().statement().to_owned(),
        reason: deferral.content().reason().to_owned(),
        reconsider_when: deferral.content().reconsider_when().to_vec(),
        deferred_at: moment(deferral.deferred_at()),
    }
}

fn intervention_state_from(intervention: &CeremonyIntervention) -> pb::CeremonyInterventionState {
    pb::CeremonyInterventionState {
        intervention_id: intervention.id().as_str().to_owned(),
        kind: intervention.kind().as_label().to_owned(),
        status: intervention.status().as_label().to_owned(),
        requested_by: intervention.requested_by().as_str().to_owned(),
        target: Some(match intervention.target().role_ids() {
            Some(role_ids) => pb::CeremonyInterventionTargetState {
                kind: "roles".to_owned(),
                role_ids: role_ids
                    .iter()
                    .map(RoleId::as_str)
                    .map(str::to_owned)
                    .collect(),
            },
            None => pb::CeremonyInterventionTargetState {
                kind: "table".to_owned(),
                role_ids: Vec::new(),
            },
        }),
        request: Some(pb::CeremonyInterventionMessage {
            message: intervention.request().message().to_owned(),
            details: Some(attributes_to_struct(intervention.request().details())),
        }),
        provenance: intervention.provenance().map(|provenance| {
            pb::CeremonyInterventionProvenanceState {
                source_intervention_id: provenance.source_intervention_id().as_str().to_owned(),
                source_response_role_id: provenance.source_response_role_id().as_str().to_owned(),
                selected_role_id: provenance.selected_role_id().as_str().to_owned(),
            }
        }),
        responses: intervention
            .responses()
            .iter()
            .map(intervention_response_state_from)
            .collect(),
        created_at: moment(intervention.created_at()),
        updated_at: moment(intervention.updated_at()),
        closed_at: intervention.closed_at().map(moment).unwrap_or_default(),
    }
}

fn intervention_response_state_from(
    response: &CeremonyInterventionResponse,
) -> pb::CeremonyInterventionResponseState {
    pb::CeremonyInterventionResponseState {
        role_id: response.role_id().as_str().to_owned(),
        content: Some(pb::CeremonyInterventionMessage {
            message: response.content().message().to_owned(),
            details: Some(attributes_to_struct(response.content().details())),
        }),
        evidence_pack: response
            .evidence_pack()
            .map(|pack| serde_json::to_string(pack).unwrap_or_default())
            .unwrap_or_default(),
        responded_at: moment(response.responded_at()),
    }
}

fn open_intervention_ids(instance: &CeremonyInstance) -> Vec<String> {
    instance
        .interventions()
        .iter()
        .filter(|intervention| intervention.status().is_open())
        .map(|intervention| intervention.id().as_str().to_owned())
        .collect()
}

/// A record reference, on its way back out.
///
/// Flat with a discriminator, mirroring the shape the request side
/// already accepts: one message that says which kind of thing it points
/// at and fills only the fields that kind uses.
fn record_ref_state_from(record: &CeremonyRecordRef) -> pb::CeremonyRecordRefState {
    match record {
        CeremonyRecordRef::Step { step_id } => pb::CeremonyRecordRefState {
            kind: "step".to_owned(),
            step_id: step_id.as_str().to_owned(),
            ..pb::CeremonyRecordRefState::default()
        },
        CeremonyRecordRef::AgendaItem { agenda_item } => pb::CeremonyRecordRefState {
            kind: "agenda_item".to_owned(),
            agenda_item: agenda_item.as_str().to_owned(),
            ..pb::CeremonyRecordRefState::default()
        },
        CeremonyRecordRef::Contribution {
            agenda_item,
            ordinal,
        } => pb::CeremonyRecordRefState {
            kind: "contribution".to_owned(),
            agenda_item: agenda_item.as_str().to_owned(),
            ordinal: *ordinal,
            ..pb::CeremonyRecordRefState::default()
        },
        CeremonyRecordRef::GuardDecision { guard_name } => pb::CeremonyRecordRefState {
            kind: "guard_decision".to_owned(),
            guard_name: guard_name.as_str().to_owned(),
            ..pb::CeremonyRecordRefState::default()
        },
        CeremonyRecordRef::Transition { ordinal } => pb::CeremonyRecordRefState {
            kind: "transition".to_owned(),
            ordinal: *ordinal,
            ..pb::CeremonyRecordRefState::default()
        },
    }
}
