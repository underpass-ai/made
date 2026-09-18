use made_core::error::DomainError;
use made_core::value_objects::{
    GuardCondition, GuardName, OutputFieldGuardCondition, RoleId, StepId, StepOutputField,
    StepStatus, TransitionTrigger,
};

use crate::usecases::ceremony_design_route::CeremonyDesignRoute;

pub(super) fn output_route(
    from: &StepId,
    to: &StepId,
    owner: &RoleId,
    completed: &StepId,
    output_step: &StepId,
    field: &str,
    expected: bool,
    label: &str,
) -> Result<CeremonyDesignRoute, DomainError> {
    route(
        from,
        to,
        owner,
        completed,
        vec![(output_step, field, serde_json::json!(expected))],
        label,
    )
}

pub(super) fn string_route(
    from: &StepId,
    to: &StepId,
    owner: &RoleId,
    step: &StepId,
    field: &str,
    expected: &str,
) -> Result<CeremonyDesignRoute, DomainError> {
    route(
        from,
        to,
        owner,
        step,
        vec![(step, field, serde_json::json!(expected))],
        &format!("to_{}", expected.to_ascii_lowercase()),
    )
}

pub(super) fn human_handoff_route(
    from: &StepId,
    to: &StepId,
    owner: &RoleId,
    step: &StepId,
    field: &str,
    expected: &str,
) -> Result<CeremonyDesignRoute, DomainError> {
    let prefix = format!("{}_to_{}", from, expected.to_ascii_lowercase());
    Ok(CeremonyDesignRoute::new(
        from.clone(),
        to.clone(),
        TransitionTrigger::new(&prefix)?,
        owner.clone(),
        vec![
            (
                GuardName::new(format!("{prefix}_completed"))?,
                GuardCondition::StepStatus {
                    step_id: step.clone(),
                    status: StepStatus::Completed,
                },
            ),
            (
                GuardName::new(format!("{prefix}_output_0"))?,
                GuardCondition::OutputField(OutputFieldGuardCondition::new(
                    step.clone(),
                    StepOutputField::new(field)?,
                    serde_json::json!(expected),
                )),
            ),
            (
                GuardName::new(format!("{prefix}_human"))?,
                GuardCondition::HumanApproval,
            ),
        ],
    ))
}

pub(super) fn two_output_route(
    from: &StepId,
    to: &StepId,
    owner: &RoleId,
    step: &StepId,
    first: &str,
    first_value: bool,
    second: &str,
    second_value: bool,
    label: &str,
) -> Result<CeremonyDesignRoute, DomainError> {
    route(
        from,
        to,
        owner,
        step,
        vec![
            (step, first, serde_json::json!(first_value)),
            (step, second, serde_json::json!(second_value)),
        ],
        label,
    )
}

fn route(
    from: &StepId,
    to: &StepId,
    owner: &RoleId,
    completed: &StepId,
    outputs: Vec<(&StepId, &str, serde_json::Value)>,
    label: &str,
) -> Result<CeremonyDesignRoute, DomainError> {
    let prefix = format!("{from}_{label}");
    let mut guards = vec![(
        GuardName::new(format!("{prefix}_completed"))?,
        GuardCondition::StepStatus {
            step_id: completed.clone(),
            status: StepStatus::Completed,
        },
    )];
    for (index, (step, field, expected)) in outputs.into_iter().enumerate() {
        guards.push((
            GuardName::new(format!("{prefix}_output_{index}"))?,
            GuardCondition::OutputField(OutputFieldGuardCondition::new(
                step.clone(),
                StepOutputField::new(field)?,
                expected,
            )),
        ));
    }
    Ok(CeremonyDesignRoute::new(
        from.clone(),
        to.clone(),
        TransitionTrigger::new(prefix)?,
        owner.clone(),
        guards,
    ))
}
