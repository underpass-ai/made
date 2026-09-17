use std::collections::BTreeSet;

use made_core::error::DomainError;

use super::{
    approval_guard_name, approval_trigger, completion_guard, exit_guard_name, num_agents,
    CeremonyDesignDocument, COMPLETED_STATE, RESERVED_ACTIONS,
};
use crate::usecases::CeremonyDesignExitGuard;

fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}

#[allow(clippy::too_many_lines)] // Every input invariant is audited in one authoring gate.
pub(super) fn validate(document: &CeremonyDesignDocument) -> Result<(), DomainError> {
    if document.outputs().is_empty() {
        return Err(invalid("field `outputs` must contain at least one output"));
    }
    if document.participants().is_empty() {
        return Err(invalid(
            "field `participants` must contain at least one participant",
        ));
    }
    if document.stages().is_empty() {
        return Err(invalid("field `stages` must contain at least one stage"));
    }
    reject_duplicates(
        document
            .required_inputs()
            .iter()
            .map(|input| input.as_str().to_owned()),
        "required_inputs",
    )?;
    reject_duplicates(
        document
            .optional_inputs()
            .iter()
            .map(|input| input.as_str().to_owned()),
        "optional_inputs",
    )?;
    reject_duplicates(
        document
            .outputs()
            .iter()
            .map(|output| output.as_str().to_owned()),
        "outputs",
    )?;
    reject_overlap(document)?;

    let participant_ids = document
        .participants()
        .iter()
        .map(|participant| participant.role_id().as_str().to_owned())
        .collect::<Vec<_>>();
    reject_duplicates(participant_ids.iter().cloned(), "participants.role_id")?;
    let participant_set = participant_ids.into_iter().collect::<BTreeSet<_>>();

    let mut stage_ids = Vec::with_capacity(document.stages().len());
    for stage in document.stages() {
        stage_ids.push(stage.id().as_str().to_owned());
        if !participant_set.contains(stage.owner_role_id().as_str()) {
            return Err(invalid(format!(
                "stage `{}` names unknown owner role `{}`",
                stage.id(),
                stage.owner_role_id()
            )));
        }
        if stage.review_rounds().get() > 0 && num_agents(stage) < 2 {
            return Err(invalid(format!(
                "stage `{}` requests review rounds with fewer than two agents",
                stage.id()
            )));
        }
        for (index, guard) in stage.exit_guards().iter().enumerate() {
            if stage.exit_guards()[..index].contains(guard) {
                return Err(invalid(format!(
                    "stage `{}` contains a duplicate exit guard",
                    stage.id()
                )));
            }
        }
    }
    reject_duplicates(stage_ids.iter().cloned(), "stages.id")?;
    let declared_steps = stage_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for stage in document.stages() {
        for guard in stage.exit_guards() {
            let step_id = match guard {
                CeremonyDesignExitGuard::OutputField(guard) => guard.step_id(),
                CeremonyDesignExitGuard::StepRepeatExhausted(guard) => guard.step_id(),
            };
            if !declared_steps.contains(step_id.as_str()) {
                return Err(invalid(format!(
                    "stage `{}` exit guard names unknown step `{step_id}`",
                    stage.id()
                )));
            }
            if let CeremonyDesignExitGuard::StepRepeatExhausted(guard) = guard {
                if guard.step_id() != stage.id() {
                    return Err(invalid(format!(
                        "stage `{}` exhausted-repeat guard must reference a step in that stage",
                        stage.id()
                    )));
                }
                if stage.repeat().is_none() {
                    return Err(invalid(format!(
                        "stage `{}` exhausted-repeat guard must reference a repeating step",
                        stage.id()
                    )));
                }
            }
        }
    }
    if stage_ids
        .iter()
        .any(|id| id.eq_ignore_ascii_case(COMPLETED_STATE))
    {
        return Err(invalid(
            "stage id `completed` is reserved for the terminal state",
        ));
    }

    let generated_triggers = completion_guards(&stage_ids);
    let generated_exit_guards = document
        .stages()
        .iter()
        .flat_map(|stage| {
            (0..stage.exit_guards().len())
                .map(move |index| exit_guard_name(stage.id().as_str(), index))
        })
        .collect::<BTreeSet<_>>();
    for stage_id in &stage_ids {
        if generated_triggers.contains(stage_id) || RESERVED_ACTIONS.contains(&stage_id.as_str()) {
            return Err(invalid(format!(
                "stage id `{stage_id}` collides with a generated transition or role capability"
            )));
        }
    }

    if let Some(approval) = document.final_approval() {
        if !participant_set.contains(approval.role_id().as_str()) {
            return Err(invalid(format!(
                "final approval names unknown role `{}`",
                approval.role_id()
            )));
        }
        let guard_name = approval_guard_name(document);
        let trigger = approval_trigger(document);
        if generated_triggers.contains(&guard_name) || generated_exit_guards.contains(&guard_name) {
            return Err(invalid(format!(
                "final approval guard `{guard_name}` collides with a generated completion guard"
            )));
        }
        if generated_triggers.contains(&trigger)
            || stage_ids.contains(&trigger)
            || RESERVED_ACTIONS.contains(&trigger.as_str())
        {
            return Err(invalid(format!(
                "final approval trigger `{trigger}` collides with a stage, generated transition or role capability"
            )));
        }
    }

    for participant in document.participants() {
        let role_id = participant.role_id().as_str();
        let owns_stage = document
            .stages()
            .iter()
            .any(|stage| stage.owner_role_id().as_str() == role_id);
        let owns_approval = document
            .final_approval()
            .is_some_and(|approval| approval.role_id().as_str() == role_id);
        if participant.capabilities().is_empty() && !owns_stage && !owns_approval {
            return Err(invalid(format!(
                "participant role `{role_id}` has no stage, approval or intervention capability"
            )));
        }
    }

    Ok(())
}

fn completion_guards(stage_ids: &[String]) -> BTreeSet<String> {
    stage_ids
        .iter()
        .map(|id| completion_guard(id))
        .collect::<BTreeSet<_>>()
}

fn reject_duplicates(
    values: impl IntoIterator<Item = String>,
    field: &str,
) -> Result<(), DomainError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            return Err(invalid(format!(
                "field `{field}` contains duplicate `{value}`"
            )));
        }
    }
    Ok(())
}

fn reject_overlap(document: &CeremonyDesignDocument) -> Result<(), DomainError> {
    let required = document
        .required_inputs()
        .iter()
        .map(made_core::value_objects::InputName::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(overlap) = document
        .optional_inputs()
        .iter()
        .map(made_core::value_objects::InputName::as_str)
        .find(|value| required.contains(value))
    {
        return Err(invalid(format!(
            "`{overlap}` appears in both `required_inputs` and `optional_inputs`"
        )));
    }
    Ok(())
}
