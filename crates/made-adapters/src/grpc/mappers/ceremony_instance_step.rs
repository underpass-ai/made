//! Step projection preserves original claim identity and reports effective expiry.
use made_app::usecases::CeremonyStepView;
use made_proto::v1 as pb;

use super::attributes::{attributes_to_struct, struct_from_json};
use super::ceremony_instance::moment;

pub(super) fn step_state_from(step: &CeremonyStepView<'_>) -> pb::CeremonyStepState {
    pb::CeremonyStepState {
        effective_lease_expires_at: step
            .record()
            .effective_lease_expires_at()
            .map(moment)
            .unwrap_or_default(),
        step_id: step.step().id().as_str().to_owned(),
        state_id: step.step().state_id().as_str().to_owned(),
        status: step.record().status().as_label().to_owned(),
        attempt: step.record().attempt().get(),
        output: Some(attributes_to_struct(step.record().output().attributes())),
        error: step
            .record()
            .error_message()
            .map(ToString::to_string)
            .unwrap_or_default(),
        iteration: step.record().iteration().get(),
        repeat_condition_satisfied: step.repeat_condition_satisfied(),
        repeat_limit_reached: step.repeat_limit_reached(),
        repeat_max_iterations: step
            .step()
            .repeat_policy()
            .map(|policy| policy.max_iterations().get())
            .unwrap_or_default(),
        state_visit: step.record().state_visit().get(),
        state_iteration: step.record().state_iteration().get(),
        execution_profile: step
            .record()
            .lease()
            .and_then(|lease| lease.execution_profile())
            .and_then(|profile| serde_json::to_value(profile).ok())
            .and_then(|value| struct_from_json(&value)),
    }
}
