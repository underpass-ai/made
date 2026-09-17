use made_app::usecases::{
    CeremonyDesignDocument, CeremonyDesignFinalApproval, CeremonyDesignParticipant,
    CeremonyDesignRepeat, CeremonyDesignStage, CeremonyParticipantCapability, DesignedCeremony,
};
use made_core::error::DomainError;
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, GuardName, InputName, OutputName, RoleId,
    StepHandlerKind, StepId, StepIteration, StepOutputField, TransitionTrigger,
};
use made_embedded::EmbeddedMade;
use serde::Deserialize;
use serde_json::Value;

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
    stages: Vec<StageIntent>,
    #[serde(default)]
    final_approval: Option<FinalApprovalIntent>,
    #[serde(default)]
    step_timeout_seconds: Option<u64>,
    #[serde(default)]
    max_attempts: Option<u32>,
    #[serde(default)]
    backoff_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParticipantIntent {
    role_id: String,
    /// The words themselves, read by the use case rather than by
    /// serde: one vocabulary, one refusal, on both backends.
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageIntent {
    id: String,
    owner_role_id: String,
    instructions: String,
    #[serde(default)]
    handler: Option<String>,
    #[serde(default)]
    see_prior: Option<bool>,
    #[serde(default)]
    num_agents: Option<u64>,
    #[serde(default)]
    review_rounds: u64,
    #[serde(default)]
    repeat: Option<RepeatIntent>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepeatIntent {
    max_iterations: u32,
    output_field: String,
    equals: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinalApprovalIntent {
    role_id: String,
    #[serde(default)]
    guard_name: Option<String>,
    #[serde(default)]
    trigger: Option<String>,
}

impl TryFrom<&Value> for EmbeddedDesignCeremonyRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        if !value.is_object() {
            return Err("tools/call.arguments must be an object".to_owned());
        }
        serde_json::from_value(value.clone())
            .map_err(|error| format!("invalid ceremony design intent: {error}"))
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

        Ok(CeremonyDesignDocument::new(
            CeremonyName::new(self.name)?,
            self.version.map(CeremonyVersion::new).transpose()?,
            CeremonyDescription::new(self.objective)?,
            names(self.required_inputs, InputName::new)?,
            names(self.optional_inputs, InputName::new)?,
            names(self.outputs, OutputName::new)?,
            participants,
            stages,
            final_approval,
            self.step_timeout_seconds,
            self.max_attempts,
            self.backoff_seconds,
        ))
    }
}

impl ParticipantIntent {
    fn into_domain(self) -> Result<CeremonyDesignParticipant, DomainError> {
        Ok(CeremonyDesignParticipant::new(
            RoleId::new(self.role_id)?,
            self.capabilities
                .iter()
                .map(|capability| CeremonyParticipantCapability::parse(capability))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }
}

impl StageIntent {
    fn into_domain(self) -> Result<CeremonyDesignStage, DomainError> {
        Ok(CeremonyDesignStage::new(
            StepId::new(self.id)?,
            RoleId::new(self.owner_role_id)?,
            self.instructions,
            self.handler.map(StepHandlerKind::new).transpose()?,
            self.see_prior,
            self.num_agents,
            self.review_rounds,
            self.repeat.map(RepeatIntent::into_domain).transpose()?,
        ))
    }
}

impl RepeatIntent {
    fn into_domain(self) -> Result<CeremonyDesignRepeat, DomainError> {
        Ok(CeremonyDesignRepeat::new(
            StepIteration::new(self.max_iterations)?,
            StepOutputField::new(self.output_field)?,
            self.equals,
        ))
    }
}

impl FinalApprovalIntent {
    fn into_domain(self) -> Result<CeremonyDesignFinalApproval, DomainError> {
        Ok(CeremonyDesignFinalApproval::new(
            RoleId::new(self.role_id)?,
            self.guard_name.map(GuardName::new).transpose()?,
            self.trigger.map(TransitionTrigger::new).transpose()?,
        ))
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
    use super::*;
    use made_adapters::yaml::CeremonyDefinitionYaml;
    use made_core::entities::CeremonyDefinitionDraft;
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
