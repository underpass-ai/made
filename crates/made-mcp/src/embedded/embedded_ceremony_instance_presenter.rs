use made_app::usecases::{CeremonyInstanceView, StartCeremonyStepOutput};
use made_core::entities::{AuditRecord, CeremonyInstance};
use made_core::value_objects::{
    CeremonyDefinitionDigest, CeremonyEndReason, CeremonyId, CeremonyInterventionTarget,
    CeremonyLineage, CeremonyRecordRef, ChildCompletionRef, ChildGroupState, PlannedChild,
    RecalledEntry, RoleId, SessionRecollection, StepDeadline, StepId,
};
use made_embedded::{EmbeddedCeremonyAuthority, EmbeddedCeremonyProjection, EmbeddedMade};
use time::OffsetDateTime;

use crate::protocol::ToolError;
use serde_json::{json, Value};

/// Projects the current persistent ceremony state onto the MCP wire contract.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct EmbeddedCeremonyInstancePresenter;

impl EmbeddedCeremonyInstancePresenter {
    pub(super) async fn present(
        made: &EmbeddedMade,
        ceremony_id: &CeremonyId,
    ) -> Result<Value, ToolError> {
        let projection = EmbeddedCeremonyAuthority::for_engine(made)
            .projection(ceremony_id)
            .await?;
        Self::render(
            made,
            projection.instance(),
            projection.definition(),
            projection.records().last(),
        )
    }

    pub(super) async fn present_claim(
        made: &EmbeddedMade,
        claim: &StartCeremonyStepOutput,
    ) -> Result<Value, ToolError> {
        let projection = EmbeddedCeremonyAuthority::for_engine(made)
            .projection(claim.instance().id())
            .await?;
        let head = projection
            .records()
            .iter()
            .find(|record| record.sequence().value() == claim.version().value());
        let mut value = Self::render(made, claim.instance(), projection.definition(), head)?;
        value["claim_fence"] = json!(claim.claim_fence().as_str());
        Ok(value)
    }

    fn render(
        made: &EmbeddedMade,
        instance: &CeremonyInstance,
        definition: &made_core::entities::CeremonyDefinition,
        head: Option<&AuditRecord>,
    ) -> Result<Value, ToolError> {
        // Derived once in the application layer and rendered here. The
        // gRPC adapter renders the same view, which is what keeps one
        // working session from looking like two depending on how a
        // client reached it.
        let view = made.project_instance(instance, definition)?;
        let steps = step_values(&view);
        let transitions = transition_values(&view);
        let waiting_for_human = view
            .waiting_for_human()
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>();
        let next_step_id = view.next_step_id().map(StepId::as_str);
        let claimable_step_ids = view
            .claimable_step_ids()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>();
        let interventions = intervention_values(instance);
        let open_intervention_ids = open_intervention_ids(instance);
        let guard_deferrals = guard_deferral_values(instance);
        let lifecycle = instance.lifecycle();
        let step_deadlines = instance
            .step_deadlines()
            .values()
            .map(step_deadline_value)
            .collect::<Vec<_>>();
        let reasons = view
            .reasons()
            .iter()
            .map(|reason| {
                json!({
                    "from": record_ref_value(reason.from()),
                    "to": record_ref_value(reason.to()),
                    "kind": reason.kind().as_label(),
                    "why": reason.why(),
                    "confidence": reason.confidence().as_label(),
                    "asserted_by_role_id": reason.asserted_by().map(RoleId::as_str),
                    "asserted_at": moment(reason.asserted_at()),
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "ceremony_id": instance.id().as_str(),
            "budget_account_id": instance
                .budget_account_id()
                .map(made_core::value_objects::BudgetAccountId::as_str),
            "trace_id": head.and_then(|record| record.trace_id()),
            "correlation_id": head
                .and_then(|record| record.correlation_id())
                .map(ToString::to_string),
            "causation_id": head
                .and_then(|record| record.causation_id())
                .map(ToString::to_string),
            "definition_name": definition.name().as_str(),
            "definition_version": definition.version().as_str(),
            // Present whether this instance runs a definition that can
            // be looked up and checked, or one supplied for the run.
            // Binding it and not saying so would leave the caller
            // unable to tell the two apart.
            "bound_definition_digest": instance
                .bound_definition()
                .map(CeremonyDefinitionDigest::to_hex),
            "current_state": instance.current_state().as_str(),
            "current_state_iteration": instance.current_state_iteration().get(),
            "current_state_visit": instance.current_state_visit().get(),
            "state_repeat_max_iterations": definition.state(instance.current_state())
                .and_then(|state| state.repeat_policy()).map(|policy| policy.max_iterations().get()),
            "state_repeat_condition_satisfied": instance.state_repeat_condition_is_satisfied(definition),
            "state_repeat_limit_reached": instance.state_repeat_limit_reached(definition),
            "completed": view.is_completed(),
            "next_step_id": next_step_id,
            "claimable_step_ids": claimable_step_ids,
            "waiting_for_human": waiting_for_human,
            "guard_deferrals": guard_deferrals,
            "transitions": transitions,
            "steps": steps,
            "interventions": interventions,
            "open_intervention_ids": open_intervention_ids,
            "context": instance.context(),
            // What this session was told when it opened, or `null` when
            // it was told nothing — which is every session that
            // declares no `memory_scope`.
            "recollection": instance.recollection().map(recollection_value),
            "lineage": instance.lineage().map(lineage_value),
            // Both directions of the relation, so "which ceremony
            // replaced this one" and "which one did this replace" are
            // answerable without reading raw streams.
            "succession": instance.succession().map(succession_value),
            "successor_plan": instance
                .successor_plan()
                .map(super::embedded_succession_dispatch::plan_value),
            "child_groups": instance.child_groups().values().map(child_group_value).collect::<Vec<_>>(),
            "lifecycle": lifecycle.phase().as_label(),
            "end_reason": lifecycle.end_reason().map(CeremonyEndReason::as_label),
            "paused_at": lifecycle.is_paused().then(|| lifecycle.changed_at().map(moment)).flatten(),
            "ended_at": lifecycle.is_ended().then(|| lifecycle.changed_at().map(moment)).flatten(),
            "ceremony_deadline_at": instance.ceremony_deadline().map(|deadline| moment(deadline.at())),
            "state_deadline_at": instance.state_deadline().map(|deadline| moment(deadline.at())),
            "step_deadlines": step_deadlines,
            "participant_bindings": view
                .participant_bindings()
                .values()
                .map(|binding| json!({
                    "role_id": binding.role_id().as_str(),
                    "specialty": binding.specialty().as_str(),
                    "bound_at": moment(binding.bound_at()),
                }))
                .collect::<Vec<_>>(),
            // Read back, not only written: a seat that explained this
            // session can find its own explanation again. Until now the
            // edges crossed the wire in one direction only.
            "reasons": reasons,
        }))
    }
}

fn transition_values(view: &CeremonyInstanceView<'_>) -> Vec<Value> {
    view.transitions()
        .iter()
        .map(|transition| {
            json!({
                "trigger": transition.transition().trigger().as_str(),
                "to_state": transition.transition().to().as_str(),
                "enabled": transition.is_enabled(),
                "guards": transition
                    .guards()
                    .iter()
                    .map(|guard| json!({
                        "name": guard.name().as_str(),
                        "kind": if guard.is_human() { "human" } else { "automated" },
                        "satisfied": guard.is_satisfied(),
                    }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn step_deadline_value(deadline: &StepDeadline) -> Value {
    json!({
        "step_id": deadline.step_id().as_str(),
        "state_visit": deadline.state_visit().get(),
        "state_iteration": deadline.state_iteration().get(),
        "step_iteration": deadline.step_iteration().get(),
        "attempt": deadline.attempt().get(),
        "claim_fence": deadline.claim_fence().as_str(),
        "deadline_at": moment(deadline.at()),
        "finished_by_role_id": deadline.finished_by().as_str(),
    })
}

pub(super) fn child_completion_value(completion: &ChildCompletionRef) -> Value {
    json!({
        "group_id": completion.group_id().as_str(),
        "child_id": completion.child_id().as_str(),
        "terminal_event_id": completion.terminal_event_id().as_str(),
        "terminal_record_hash": completion.terminal_record_hash().to_hex(),
    })
}

fn lineage_value(lineage: &CeremonyLineage) -> Value {
    json!({
        "root_id": lineage.root_id().as_str(),
        "parent_id": lineage.parent_id().as_str(),
        "group_id": lineage.group_id().as_str(),
        "position": lineage.position().get(),
        "depth": lineage.depth().get(),
        "remaining_depth": lineage.remaining_depth().get(),
    })
}

fn planned_child_value(child: &PlannedChild) -> Value {
    json!({
        "child_id": child.child_id().as_str(),
        "position": child.position().get(),
        "ceremony": child.ceremony().as_str(),
        "version": child.version().as_str(),
        "definition_digest": child.digest().to_hex(),
        "context": child.context(),
        "lineage": lineage_value(child.lineage()),
        "recollection": child.recollection().map(recollection_value),
        "opened_at": moment(child.opened_at()),
    })
}

fn child_group_value(group: &ChildGroupState) -> Value {
    let plan = group.plan();
    let coordinates = plan.coordinates();
    json!({
        "group_id": plan.group_id().as_str(),
        "step_id": coordinates.step_id().as_str(),
        "state_visit": coordinates.state_visit().get(),
        "state_iteration": coordinates.state_iteration().get(),
        "step_iteration": coordinates.step_iteration().get(),
        "active_claim_fence": plan.active_claim_fence().as_str(),
        "adopted_claim_fence": group.adopted_claim_fence().as_str(),
        "max_children": plan.max_children().get(),
        "max_depth": plan.max_depth().get(),
        "children": plan.children().iter().map(planned_child_value).collect::<Vec<_>>(),
        "completions": group.completions().values().map(child_completion_value).collect::<Vec<_>>(),
    })
}

/// Which ceremony this one succeeds, when it succeeds one.
fn succession_value(succession: &made_core::value_objects::CeremonySuccession) -> Value {
    json!({
        "predecessor_id": succession.predecessor_id().as_str(),
        "predecessor_head": succession.predecessor_head().to_hex(),
        "predecessor_version": succession.predecessor_version().value(),
        "predecessor_definition": {
            "name": succession.predecessor_definition().name().as_str(),
            "version": succession.predecessor_definition().version().as_str(),
            "digest": succession.predecessor_definition().digest().to_hex(),
        },
        "successor_definition": {
            "name": succession.successor_definition().name().as_str(),
            "version": succession.successor_definition().version().as_str(),
            "digest": succession.successor_definition().digest().to_hex(),
        },
        "plan_id": succession.plan_id().as_str(),
    })
}

fn step_values(view: &CeremonyInstanceView<'_>) -> Vec<Value> {
    view.steps()
        .iter()
        .map(|step| {
            json!({
                "step_id": step.step().id().as_str(),
                "state_id": step.step().state_id().as_str(),
                "status": step.record().status().as_label(),
                "attempt": step.record().attempt().get(),
                "output": step.record().output().attributes().as_map(),
                "error": step.record().error_message().map(ToString::to_string),
                "iteration": step.record().iteration().get(),
                "state_iteration": step.record().state_iteration().get(),
                "state_visit": step.record().state_visit().get(),
                "execution_profile": step
                    .record()
                    .lease()
                    .and_then(|lease| lease.execution_profile())
                    .and_then(|profile| serde_json::to_value(profile).ok()),
                "carried_from": step
                    .carried_from()
                    .map(super::embedded_succession_dispatch::source),
                "repeat_condition_satisfied": step.repeat_condition_satisfied(),
                "repeat_limit_reached": step.repeat_limit_reached(),
                "effective_lease_expires_at": step.record().effective_lease_expires_at().map(|at| at.format(&time::format_description::well_known::Rfc3339).unwrap_or_default()),
                "repeat_max_iterations": step
                    .step()
                    .repeat_policy()
                    .map(|policy| policy.max_iterations().get()),
            })
        })
        .collect()
}

/// What earlier sessions in this scope decided, as this one was told.
fn recollection_value(recollection: &SessionRecollection) -> Value {
    json!({
        "scope": recollection.scope().as_str(),
        "truncated": recollection.completeness().is_truncated(),
        "entries": recollection
            .entries()
            .iter()
            .map(recalled_entry_value)
            .collect::<Vec<_>>(),
    })
}

fn recalled_entry_value(entry: &RecalledEntry) -> Value {
    json!({
        "entry_id": entry.id().as_str(),
        "kind": entry.kind().as_label(),
        "summary": entry.summary(),
        "from_ceremony_id": entry.from_ceremony().as_str(),
        "observed_at": moment(entry.observed_at()),
    })
}

/// What a reason points at, flat with a discriminator.
fn record_ref_value(record: &CeremonyRecordRef) -> Value {
    match record {
        CeremonyRecordRef::Step { step_id } => json!({
            "kind": "step", "step_id": step_id.as_str()
        }),
        CeremonyRecordRef::AgendaItem { agenda_item } => json!({
            "kind": "agenda_item", "agenda_item": agenda_item.as_str()
        }),
        CeremonyRecordRef::Contribution {
            agenda_item,
            ordinal,
        } => json!({
            "kind": "contribution", "agenda_item": agenda_item.as_str(), "ordinal": ordinal
        }),
        CeremonyRecordRef::GuardDecision { guard_name } => json!({
            "kind": "guard_decision", "guard_name": guard_name.as_str()
        }),
        CeremonyRecordRef::Transition { ordinal } => json!({
            "kind": "transition", "ordinal": ordinal
        }),
    }
}

fn guard_deferral_values(instance: &CeremonyInstance) -> Vec<Value> {
    instance
        .guard_deferrals()
        .iter()
        .map(|deferral| {
            json!({
                "guard_name": deferral.guard_name().as_str(),
                "statement": deferral.content().statement(),
                "reason": deferral.content().reason(),
                "reconsider_when": deferral.content().reconsider_when(),
                "deferred_at": moment(deferral.deferred_at()),
            })
        })
        .collect()
}

/// A target names what it names, and leaves out what it does not.
///
/// An item put to the whole table carries no roles and says so by
/// having no `role_ids` key at all, because an empty list reads as "put
/// to nobody", which is the one thing a target can never mean. The
/// three fields of the exact shape follow the same rule and are present
/// together or not at all.
fn intervention_target_value(target: &CeremonyInterventionTarget) -> Value {
    let mut value = json!({ "kind": target.kind_str() });
    let object = value
        .as_object_mut()
        .expect("a target renders as an object");
    if let Some(role_ids) = target.role_ids() {
        object.insert(
            "role_ids".to_owned(),
            json!(role_ids.iter().map(RoleId::as_str).collect::<Vec<_>>()),
        );
    }
    if let Some(recipient) = target.exact_recipient() {
        object.insert(
            "agent_execution_id".to_owned(),
            json!(recipient.agent_execution_id().as_str()),
        );
        object.insert(
            "incarnation".to_owned(),
            json!(recipient.incarnation().as_str()),
        );
        object.insert("role_id".to_owned(), json!(recipient.role_id().as_str()));
    }
    value
}

fn intervention_values(instance: &CeremonyInstance) -> Vec<Value> {
    instance
        .interventions()
        .iter()
        .map(|intervention| {
            // One shape for the three: the versioned contract carries
            // all five fields and leaves the ones a target does not use
            // empty, so a caller reads the same keys whichever engine
            // answered and whichever kind of target it is.
            let target = intervention_target_value(intervention.target());
            let responses = intervention
                .responses()
                .iter()
                .map(|response| {
                    let mut value = json!({
                        "role_id": response.role_id().as_str(),
                        "message": response.content().message(),
                        "details": response.content().details().as_map(),
                        "evidence_pack": response.evidence_pack(),
                        "responded_at": moment(response.responded_at()),
                    });
                    // Who answered and which offer they closed, when an
                    // agent was handed the item. Absent for a seat that
                    // answered without being handed anything, rather
                    // than three nulls saying the same.
                    if let (Some(executor), Some(delivery_id)) =
                        (response.executor(), response.delivery_id())
                    {
                        let object = value.as_object_mut().expect("a response is an object");
                        object.insert(
                            "executor_agent_execution_id".to_owned(),
                            json!(executor.agent_execution_id().as_str()),
                        );
                        object.insert(
                            "executor_incarnation".to_owned(),
                            json!(executor.incarnation().as_str()),
                        );
                        object.insert("delivery_id".to_owned(), json!(delivery_id.as_str()));
                    }
                    value
                })
                .collect::<Vec<_>>();
            let provenance = intervention.provenance().map(|provenance| {
                json!({
                    "source_intervention_id": provenance.source_intervention_id().as_str(),
                    "source_response_role_id": provenance.source_response_role_id().as_str(),
                    "selected_role_id": provenance.selected_role_id().as_str(),
                })
            });
            json!({
                "intervention_id": intervention.id().as_str(),
                "kind": intervention.kind().as_label(),
                "status": intervention.status().as_label(),
                "requested_by": intervention.requested_by().as_str(),
                "target": target,
                "request": {
                    "message": intervention.request().message(),
                    "details": intervention.request().details().as_map(),
                },
                "provenance": provenance,
                "responses": responses,
                "created_at": moment(intervention.created_at()),
                "updated_at": moment(intervention.updated_at()),
                "closed_at": intervention.closed_at().map_or(Value::Null, moment),
            })
        })
        .collect()
}

fn open_intervention_ids(instance: &CeremonyInstance) -> Vec<&str> {
    instance
        .interventions()
        .iter()
        .filter(|intervention| intervention.status().is_open())
        .map(|intervention| intervention.id().as_str())
        .collect()
}

/// One instant, one rendering.
///
/// `time`'s own serialization writes `2026-09-16 09:00:00.0 +00:00:00`,
/// which is neither RFC 3339 nor what the gRPC arm renders from the
/// same view, so the same moment read two ways depending on which
/// engine served the call. Both arms now answer RFC 3339.
fn moment(at: OffsetDateTime) -> Value {
    at.format(&time::format_description::well_known::Rfc3339)
        .map_or(Value::Null, Value::String)
}
