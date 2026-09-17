use made_app::usecases::{CeremonyDesignDocument, CeremonyPatternPreset, DesignedCeremony};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, DurationMs, InputName, OutputName,
    StepAttempt, StepTimeout,
};
use made_embedded::EmbeddedMade;
use serde::Deserialize;
use serde_json::Value;

mod exit_guard_intent;
mod final_approval_intent;
mod output_field_guard_intent;
mod participant_intent;
mod repeat_intent;
mod stage_intent;
mod step_repeat_exhausted_guard_intent;

use exit_guard_intent::ExitGuardIntent;
use final_approval_intent::FinalApprovalIntent;
use output_field_guard_intent::OutputFieldGuardIntent;
use participant_intent::ParticipantIntent;
use repeat_intent::RepeatIntent;
use stage_intent::StageIntent;
use step_repeat_exhausted_guard_intent::StepRepeatExhaustedGuardIntent;

/// Structured intent accepted by `made_design_ceremony`.
///
/// A serde shape over
/// [`made_app::usecases::CeremonyDesignDocument`] and nothing else:
/// the host's JSON becomes value objects here, and every decision
/// about what that intent means — the topology, what an omitted field
/// stands for, which intents cannot become a ceremony — belongs to
/// [`made_app::usecases::DesignCeremonyUseCase`], so that the same
/// document designs the same ceremony whichever surface took it.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmbeddedDesignCeremonyRequest {
    name: String,
    #[serde(default)]
    version: Option<String>,
    objective: String,
    #[serde(default)]
    required_inputs: Vec<String>,
    #[serde(default)]
    optional_inputs: Vec<String>,
    outputs: Vec<String>,
    participants: Vec<ParticipantIntent>,
    #[serde(default)]
    stages: Vec<StageIntent>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    final_approval: Option<FinalApprovalIntent>,
    #[serde(default)]
    step_timeout_seconds: Option<u64>,
    #[serde(default)]
    max_attempts: Option<u32>,
    #[serde(default)]
    backoff_seconds: Option<u64>,
}

impl TryFrom<&Value> for EmbeddedDesignCeremonyRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        validate_pattern(object)?;
        serde_json::from_value(value.clone())
            .map_err(|error| format!("invalid ceremony design intent: {error}"))
    }
}

fn validate_pattern(object: &serde_json::Map<String, Value>) -> Result<(), String> {
    match object.get("pattern") {
        None => Ok(()),
        Some(Value::String(pattern)) if !pattern.is_empty() => Ok(()),
        Some(Value::String(_)) => Err("`pattern` must not be empty".to_owned()),
        Some(_) => Err("`pattern` must be a string".to_owned()),
    }
}

impl EmbeddedDesignCeremonyRequest {
    pub(super) fn execute(self, made: &EmbeddedMade) -> Result<DesignedCeremony, DomainError> {
        made.design(&self.into_document()?)
    }

    fn into_document(self) -> Result<CeremonyDesignDocument, DomainError> {
        let participants = self
            .participants
            .into_iter()
            .map(ParticipantIntent::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let stages = self
            .stages
            .into_iter()
            .map(StageIntent::into_domain)
            .collect::<Result<Vec<_>, _>>()?;
        let final_approval = self
            .final_approval
            .map(FinalApprovalIntent::into_domain)
            .transpose()?;

        let document = CeremonyDesignDocument::new(
            CeremonyName::new(self.name)?,
            self.version.map(CeremonyVersion::new).transpose()?,
            CeremonyDescription::new(self.objective)?,
            names(self.required_inputs, InputName::new)?,
            names(self.optional_inputs, InputName::new)?,
            names(self.outputs, OutputName::new)?,
            participants,
            stages,
            final_approval,
            self.step_timeout_seconds
                .map(|seconds| {
                    StepTimeout::new(DurationMs::from_millis(seconds.saturating_mul(1_000)))
                })
                .transpose()?,
            self.max_attempts.map(StepAttempt::new).transpose()?,
            self.backoff_seconds
                .map(|seconds| DurationMs::from_millis(seconds.saturating_mul(1_000))),
        );
        let pattern = self
            .pattern
            .map(|pattern| CeremonyPatternPreset::parse(&pattern))
            .transpose()?;
        Ok(pattern.map_or(document.clone(), |pattern| document.with_pattern(pattern)))
    }
}

fn names<T>(
    values: Vec<String>,
    constructor: impl Fn(String) -> Result<T, DomainError>,
) -> Result<Vec<T>, DomainError> {
    values.into_iter().map(constructor).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use made_adapters::yaml::CeremonyDefinitionYaml;
    use made_core::entities::CeremonyDefinitionDraft;
    use made_core::value_objects::{GuardCondition, GuardName, StateId, StepId};
    use serde_json::json;

    fn intent() -> Value {
        json!({
            "name": "art_review",
            "objective": "Compose one candidate and ask the artist to accept it.",
            "required_inputs": ["brief", "parts"],
            "outputs": ["candidate_review"],
            "participants": [
                { "role_id": "WORKER", "capabilities": ["respond_to_intervention"] },
                { "role_id": "ARTIST", "capabilities": ["request_intervention"] }
            ],
            "stages": [
                {
                    "id": "compose",
                    "owner_role_id": "WORKER",
                    "instructions": "Compose the candidate."
                },
                {
                    "id": "review",
                    "owner_role_id": "ARTIST",
                    "instructions": "Review the candidate.",
                    "num_agents": 2,
                    "review_rounds": 1
                }
            ],
            "final_approval": { "role_id": "ARTIST" }
        })
    }

    fn design(value: &Value) -> Result<DesignedCeremony, DomainError> {
        EmbeddedDesignCeremonyRequest::try_from(value)
            .expect("the intent should deserialize")
            .execute(&EmbeddedMade::default())
    }

    /// The whole point of rendering YAML: it goes back through the
    /// same parser every hand-authored draft does.
    fn parsed(designed: &DesignedCeremony) -> CeremonyDefinitionDraft {
        CeremonyDefinitionYaml::parse_draft_str(
            &made_adapters::yaml::DesignedCeremonyYaml::render(designed).unwrap(),
        )
        .expect("a designed draft parses")
    }

    #[test]
    fn designs_a_publishable_draft_with_a_real_human_guard() {
        let designed = design(&intent()).unwrap();
        let draft = parsed(&designed);
        let report = draft.analyze();

        assert!(report.is_valid(), "{:?}", report.findings());
        assert!(made_adapters::yaml::DesignedCeremonyYaml::render(&designed)
            .unwrap()
            .contains("type: human"));
        assert!(made_adapters::yaml::DesignedCeremonyYaml::render(&designed)
            .unwrap()
            .contains("check: manual_approval"));
        assert!(made_adapters::yaml::DesignedCeremonyYaml::render(&designed)
            .unwrap()
            .contains("rounds: 1"));
        assert!(designed.final_approval_required());
    }

    #[test]
    fn designs_a_bounded_repeating_stage() {
        let mut value = intent();
        value["stages"][0]["repeat"] = json!({
            "max_iterations": 5,
            "output_field": "ready",
            "equals": true
        });

        let designed = design(&value).unwrap();
        let yaml = made_adapters::yaml::DesignedCeremonyYaml::render(&designed).unwrap();

        assert!(yaml.contains("max_iterations: 5"), "{yaml}");
        assert!(yaml.contains("output_field: ready"), "{yaml}");
        assert!(parsed(&designed)
            .steps()
            .iter()
            .find(|step| step.id() == &StepId::new("compose").unwrap())
            .unwrap()
            .repeat_policy()
            .is_some());
    }

    #[test]
    fn designs_output_and_exhausted_guards_as_one_conjunction() {
        let mut value = intent();
        value["stages"][1]["repeat"] = json!({
            "max_iterations": 3,
            "output_field": "ready",
            "equals": true
        });
        value["stages"][1]["exit_guards"] = json!([
            {
                "kind": "output_field",
                "step": "compose",
                "output_field": "decision=key",
                "equals": {"answer": "left=right"}
            },
            {"kind": "step_repeat_exhausted", "step": "review"}
        ]);

        let designed = design(&value).unwrap();
        let yaml = made_adapters::yaml::DesignedCeremonyYaml::render(&designed).unwrap();
        let draft = parsed(&designed);

        assert!(yaml.contains("output_field:compose:decision=key={\"answer\":\"left=right\"}"));
        assert!(yaml.contains("step_repeat_exhausted:review"));
        assert!(matches!(
            draft
                .guards()
                .iter()
                .find(|guard| guard.name() == &GuardName::new("exit_review_1").unwrap())
                .unwrap()
                .condition(),
            GuardCondition::OutputField(condition)
                if condition.step_id() == &StepId::new("compose").unwrap()
        ));
        assert!(matches!(
            draft
                .guards()
                .iter()
                .find(|guard| guard.name() == &GuardName::new("exit_review_2").unwrap())
                .unwrap()
                .condition(),
            GuardCondition::StepRepeatExhausted(condition)
                if condition.step_id() == &StepId::new("review").unwrap()
        ));
        let final_transition = draft
            .transitions()
            .iter()
            .find(|transition| transition.from() == &StateId::new("REVIEW").unwrap())
            .unwrap();
        assert_eq!(
            final_transition.required_guards(),
            &[
                GuardName::new("review_completed").unwrap(),
                GuardName::new("exit_review_1").unwrap(),
                GuardName::new("exit_review_2").unwrap(),
                GuardName::new("human_approved_outcome").unwrap(),
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn refuses_unknown_duplicate_and_unscoped_exit_guards() {
        let mut unknown = intent();
        unknown["stages"][1]["exit_guards"] = json!([{
            "kind": "output_field",
            "step": "missing",
            "output_field": "ready",
            "equals": true
        }]);
        assert!(design(&unknown)
            .unwrap_err()
            .to_string()
            .contains("unknown step `missing`"));

        let guard = json!({
            "kind": "output_field",
            "step": "compose",
            "output_field": "ready",
            "equals": true
        });
        let mut duplicate = intent();
        duplicate["stages"][1]["exit_guards"] = json!([guard.clone(), guard]);
        assert!(design(&duplicate)
            .unwrap_err()
            .to_string()
            .contains("duplicate exit guard"));

        let mut wrong_stage = intent();
        wrong_stage["stages"][0]["repeat"] = json!({
            "max_iterations": 3,
            "output_field": "ready",
            "equals": true
        });
        wrong_stage["stages"][1]["exit_guards"] = json!([{
            "kind": "step_repeat_exhausted",
            "step": "compose"
        }]);
        assert!(design(&wrong_stage)
            .unwrap_err()
            .to_string()
            .contains("must reference a step in that stage"));
    }

    #[test]
    fn output_guard_requires_equals_but_accepts_explicit_null() {
        let mut missing = intent();
        missing["stages"][0]["exit_guards"] = json!([{
            "kind": "output_field",
            "step": "compose",
            "output_field": "answer"
        }]);
        assert!(EmbeddedDesignCeremonyRequest::try_from(&missing)
            .unwrap_err()
            .contains("equals"));

        let mut explicit_null = intent();
        explicit_null["stages"][0]["exit_guards"] = json!([{
            "kind": "output_field",
            "step": "compose",
            "output_field": "answer",
            "equals": null
        }]);
        let designed = design(&explicit_null).unwrap();
        let yaml = made_adapters::yaml::DesignedCeremonyYaml::render(&designed).unwrap();
        assert!(yaml.contains("output_field:compose:answer=null"), "{yaml}");
    }

    #[test]
    fn refuses_an_unbounded_repeating_stage() {
        let mut value = intent();
        value["stages"][0]["repeat"] = json!({
            "output_field": "ready",
            "equals": true
        });

        let error = EmbeddedDesignCeremonyRequest::try_from(&value).unwrap_err();

        assert!(error.contains("max_iterations"), "{error}");
    }

    #[test]
    fn refuses_a_stage_owned_by_nobody_at_the_table() {
        let mut value = intent();
        value["stages"][0]["owner_role_id"] = json!("MISSING");

        let error = design(&value).unwrap_err().to_string();

        assert!(error.contains("unknown owner role"), "{error}");
    }

    #[test]
    fn refuses_review_rounds_that_would_silently_do_nothing() {
        let mut value = intent();
        value["stages"][1]["num_agents"] = json!(1);

        let error = design(&value).unwrap_err().to_string();

        assert!(error.contains("fewer than two agents"), "{error}");
    }

    #[test]
    fn refuses_names_that_would_make_role_actions_ambiguous() {
        let mut value = intent();
        value["stages"][0]["id"] = json!("request_intervention");

        let error = design(&value).unwrap_err().to_string();

        assert!(error.contains("role capability"), "{error}");
    }

    #[test]
    fn refuses_a_participant_with_no_possible_action() {
        let mut value = intent();
        value["participants"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "role_id": "OBSERVER" }));

        let error = design(&value).unwrap_err().to_string();

        assert!(error.contains("has no stage"), "{error}");
    }

    /// A field no value object accepts is refused where it is read,
    /// not where it is used: the message names the field, not the
    /// design.
    #[test]
    fn refuses_a_field_no_value_object_accepts() {
        let mut value = intent();
        value["participants"][0]["role_id"] = json!("  ");

        let error = design(&value).unwrap_err();

        assert!(matches!(error, DomainError::EmptyField { field } if field == "role_id"));
    }
}
