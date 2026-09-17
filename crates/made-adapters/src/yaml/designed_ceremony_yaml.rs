use made_app::usecases::DesignedCeremony;
use made_core::entities::CeremonyDefinitionDraft;
use made_core::error::DomainError;
use made_core::value_objects::{GuardCondition, RepeatUntilCondition, RoleAction};
use serde_json::json;
use std::collections::BTreeMap;

mod ceremony_document;
mod guard_document;
mod inputs_document;
mod repeat_until_document;
mod retry_policies_document;
mod retry_policy_document;
mod role_document;
mod state_document;
mod step_document;
mod step_repeat_document;
mod timeouts_document;
mod transition_document;

use ceremony_document::CeremonyDocument;
use guard_document::GuardDocument;
use inputs_document::InputsDocument;
use repeat_until_document::RepeatUntilDocument;
use retry_policies_document::RetryPoliciesDocument;
use retry_policy_document::RetryPolicyDocument;
use role_document::RoleDocument;
use state_document::StateDocument;
use step_document::StepDocument;
use step_repeat_document::StepRepeatDocument;
use timeouts_document::TimeoutsDocument;
use transition_document::TransitionDocument;

/// One YAML encoding for the definition produced by the application designer.
#[derive(Debug, Default, Clone, Copy)]
pub struct DesignedCeremonyYaml;

impl DesignedCeremonyYaml {
    pub fn render(designed: &DesignedCeremony) -> Result<String, DomainError> {
        let draft = designed.definition();
        let first_step = draft.steps().first().ok_or(DomainError::EmptyCollection {
            field: "designed.steps",
        })?;
        let states = draft
            .states()
            .iter()
            .map(|state| StateDocument {
                id: state.id().as_str().to_owned(),
                initial: state.is_initial(),
                terminal: state.is_terminal(),
            })
            .collect();
        let transitions = draft
            .transitions()
            .iter()
            .map(|transition| TransitionDocument {
                from: transition.from().as_str().to_owned(),
                to: transition.to().as_str().to_owned(),
                trigger: transition.trigger().as_str().to_owned(),
                guards: transition
                    .required_guards()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            })
            .collect();
        let document = CeremonyDocument {
            version: draft.version().as_str().to_owned(),
            name: draft.name().as_str().to_owned(),
            description: draft
                .description()
                .map_or_else(String::new, |value| value.as_str().to_owned()),
            inputs: InputsDocument {
                required: draft
                    .inputs()
                    .iter()
                    .filter(|input| input.requirement().is_required())
                    .map(|input| input.name().as_str().to_owned())
                    .collect(),
                optional: draft
                    .inputs()
                    .iter()
                    .filter(|input| !input.requirement().is_required())
                    .map(|input| input.name().as_str().to_owned())
                    .collect(),
            },
            outputs: draft
                .outputs()
                .iter()
                .map(|output| (output.name().as_str().to_owned(), json!({"type":"object"})))
                .collect(),
            states,
            transitions,
            steps: steps(draft),
            guards: guards(draft)?,
            roles: roles(draft),
            timeouts: TimeoutsDocument {
                step_default: first_step
                    .timeout()
                    .map_or(0, |value| value.duration().get().div_ceil(1000)),
            },
            retry_policies: RetryPoliciesDocument {
                default: RetryPolicyDocument {
                    max_attempts: first_step.retry_policy().max_attempts().get(),
                    backoff_seconds: first_step.retry_policy().backoff().get().div_ceil(1000),
                },
            },
        };
        serde_yaml::to_string(&document).map_err(|_| DomainError::InvalidDocument {
            reason: "ceremony draft could not be rendered as YAML".to_owned(),
        })
    }
}

fn guards(draft: &CeremonyDefinitionDraft) -> Result<BTreeMap<String, GuardDocument>, DomainError> {
    draft
        .guards()
        .iter()
        .map(|guard| {
            let (guard_type, check) = match guard.condition() {
                GuardCondition::Always => ("automated", "always".to_owned()),
                GuardCondition::AllStepsCompleted => {
                    ("automated", "all_steps_completed".to_owned())
                }
                GuardCondition::StepStatus { step_id, status } => (
                    "automated",
                    format!(
                        "step_status:{step_id}:{}",
                        status.as_label().to_ascii_uppercase()
                    ),
                ),
                GuardCondition::OutputField(condition) => (
                    "automated",
                    format!(
                        "output_field:{}:{}={}",
                        condition.step_id(),
                        condition.output_field(),
                        serde_json::to_string(condition.expected()).map_err(|_| {
                            DomainError::InvalidDocument {
                                reason: "ceremony output-field guard could not be rendered"
                                    .to_owned(),
                            }
                        })?
                    ),
                ),
                GuardCondition::StepRepeatExhausted(condition) => (
                    "automated",
                    format!("step_repeat_exhausted:{}", condition.step_id()),
                ),
                GuardCondition::HumanApproval => ("human", "manual_approval".to_owned()),
            };
            Ok((
                guard.name().as_str().to_owned(),
                GuardDocument {
                    guard_type: guard_type.to_owned(),
                    check,
                },
            ))
        })
        .collect()
}

fn steps(draft: &CeremonyDefinitionDraft) -> Vec<StepDocument> {
    draft
        .steps()
        .iter()
        .map(|step| StepDocument {
            id: step.id().as_str().to_owned(),
            state: step.state_id().as_str().to_owned(),
            handler: step.handler_kind().as_str().to_owned(),
            config: step.handler_config().attributes().as_map().clone(),
            repeat: step.repeat_policy().map(|repeat| {
                let RepeatUntilCondition::OutputFieldEquals { field, expected } = repeat.until();
                StepRepeatDocument {
                    max_iterations: repeat.max_iterations().get(),
                    until: RepeatUntilDocument {
                        output_field: field.as_str().to_owned(),
                        equals: expected.clone(),
                    },
                }
            }),
        })
        .collect()
}

fn roles(draft: &CeremonyDefinitionDraft) -> Vec<RoleDocument> {
    draft
        .roles()
        .iter()
        .map(|role| {
            let mut actions = role
                .allowed_actions()
                .iter()
                .map(|action| match action {
                    RoleAction::Step(id) => id.as_str().to_owned(),
                    RoleAction::Transition(trigger) => trigger.as_str().to_owned(),
                    RoleAction::RequestIntervention => "request_intervention".to_owned(),
                    RoleAction::RespondToIntervention => "respond_to_intervention".to_owned(),
                })
                .collect::<Vec<_>>();
            actions.sort();
            RoleDocument {
                id: role.id().as_str().to_owned(),
                allowed_actions: actions,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use made_app::usecases::{
        CeremonyDesignDocument, CeremonyDesignParticipant, CeremonyDesignRepeat,
        CeremonyDesignStage, CeremonyParticipantCapability, DesignCeremonyUseCase,
    };
    use made_core::value_objects::{
        CeremonyDescription, CeremonyName, DurationMs, InputName, OutputName, RoleId, Rounds,
        StepAttempt, StepId, StepInstructions, StepIteration, StepOutputField, StepTimeout,
    };

    fn intent(seconds: u64) -> CeremonyDesignDocument {
        CeremonyDesignDocument::new(
            CeremonyName::new("review").unwrap(),
            None,
            CeremonyDescription::new("Review the supplied evidence").unwrap(),
            vec![InputName::new("evidence").unwrap()],
            vec![InputName::new("background").unwrap()],
            vec![OutputName::new("decision").unwrap()],
            vec![CeremonyDesignParticipant::new(
                RoleId::new("REVIEWER").unwrap(),
                vec![CeremonyParticipantCapability::RequestIntervention],
            )],
            vec![CeremonyDesignStage::new(
                StepId::new("review").unwrap(),
                RoleId::new("REVIEWER").unwrap(),
                StepInstructions::new("Review evidence").unwrap(),
                None,
                None,
                None,
                Rounds::ZERO,
                Some(CeremonyDesignRepeat::new(
                    StepIteration::new(3).unwrap(),
                    StepOutputField::new("ready").unwrap(),
                    json!(true),
                )),
            )],
            None,
            Some(StepTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1_000))).unwrap()),
            Some(StepAttempt::new(2).unwrap()),
            Some(DurationMs::from_millis(seconds.saturating_mul(1_000))),
        )
    }

    #[test]
    fn yaml_round_trip_preserves_definition_including_saturated_durations() {
        for seconds in [300, u64::MAX] {
            let designed = DesignCeremonyUseCase::new()
                .execute(&intent(seconds))
                .unwrap();
            let yaml = DesignedCeremonyYaml::render(&designed).unwrap();
            let parsed = super::super::CeremonyDefinitionYaml::parse_draft_str(&yaml).unwrap();
            assert_eq!(
                parsed.publish().unwrap(),
                designed.definition().clone().publish().unwrap()
            );
            assert!(yaml.contains("max_iterations: 3"));
            assert!(yaml.contains("output_field: ready"));
            assert!(
                !yaml.contains("handler_kind:"),
                "internal serde names must not leak into authoring YAML"
            );
            assert_eq!(yaml, DesignedCeremonyYaml::render(&designed).unwrap());
        }
    }
}
