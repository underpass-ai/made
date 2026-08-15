use std::collections::{BTreeMap, BTreeSet};

use made_adapters::yaml::CeremonyDefinitionYaml;
use made_core::entities::CeremonyDefinitionDraft;
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, CeremonyVersion, GuardName, InputName, OutputName, RoleId,
    StepHandlerKind, StepId, TransitionTrigger,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The result of turning structured authoring intent into an analysable draft.
#[derive(Debug)]
pub(super) struct DesignedCeremonyDraft {
    definition_yaml: String,
    draft: CeremonyDefinitionDraft,
    stage_count: usize,
    participant_count: usize,
    final_approval_required: bool,
}

impl DesignedCeremonyDraft {
    pub(super) fn definition_yaml(&self) -> &str {
        &self.definition_yaml
    }

    pub(super) fn draft(&self) -> &CeremonyDefinitionDraft {
        &self.draft
    }

    pub(super) const fn stage_count(&self) -> usize {
        self.stage_count
    }

    pub(super) const fn participant_count(&self) -> usize {
        self.participant_count
    }

    pub(super) const fn final_approval_required(&self) -> bool {
        self.final_approval_required
    }
}

/// Structured intent accepted by `made_design_ceremony`.
///
/// The host chooses the meaning — objective, participants and stages. The
/// adapter owns the mechanical topology: one state and automated completion
/// guard per stage, optional final human approval, role actions, retry policy
/// and YAML rendering. The generated document still passes through the same
/// parser and analyser as every hand-authored draft.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EmbeddedDesignCeremonyRequest {
    name: String,
    #[serde(default = "default_version")]
    version: String,
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
    #[serde(default = "default_step_timeout_seconds")]
    step_timeout_seconds: u64,
    #[serde(default = "default_max_attempts")]
    max_attempts: u32,
    #[serde(default = "default_backoff_seconds")]
    backoff_seconds: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParticipantIntent {
    role_id: String,
    #[serde(default)]
    capabilities: Vec<ParticipantCapability>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ParticipantCapability {
    RequestIntervention,
    RespondToIntervention,
}

impl ParticipantCapability {
    const fn as_action(self) -> &'static str {
        match self {
            Self::RequestIntervention => "request_intervention",
            Self::RespondToIntervention => "respond_to_intervention",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageIntent {
    id: String,
    owner_role_id: String,
    instructions: String,
    #[serde(default = "default_handler")]
    handler: String,
    #[serde(default)]
    see_prior: Option<bool>,
    #[serde(default = "default_num_agents")]
    num_agents: u64,
    #[serde(default)]
    review_rounds: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FinalApprovalIntent {
    role_id: String,
    #[serde(default = "default_approval_guard")]
    guard_name: String,
    #[serde(default = "default_approval_trigger")]
    trigger: String,
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
    pub(super) fn design(self) -> Result<DesignedCeremonyDraft, String> {
        self.validate_intent()?;

        let stage_count = self.stages.len();
        let participant_count = self.participants.len();
        let final_approval_required = self.final_approval.is_some();
        let document = self.into_document();
        let definition_yaml = serde_yaml::to_string(&document)
            .map_err(|error| format!("ceremony draft could not be rendered as YAML: {error}"))?;
        let draft = CeremonyDefinitionYaml::parse_draft_str(&definition_yaml)
            .map_err(|error| format!("designed ceremony draft could not be parsed: {error}"))?;

        Ok(DesignedCeremonyDraft {
            definition_yaml,
            draft,
            stage_count,
            participant_count,
            final_approval_required,
        })
    }

    #[allow(clippy::too_many_lines)] // Every input invariant is audited in one authoring gate.
    fn validate_intent(&self) -> Result<(), String> {
        CeremonyName::new(&self.name).map_err(|error| error.to_string())?;
        CeremonyVersion::new(&self.version).map_err(|error| error.to_string())?;
        CeremonyDescription::new(&self.objective).map_err(|error| error.to_string())?;
        if self.outputs.is_empty() {
            return Err("field `outputs` must contain at least one output".to_owned());
        }
        if self.participants.is_empty() {
            return Err("field `participants` must contain at least one participant".to_owned());
        }
        if self.stages.is_empty() {
            return Err("field `stages` must contain at least one stage".to_owned());
        }
        if self.step_timeout_seconds == 0 {
            return Err("field `step_timeout_seconds` must be greater than zero".to_owned());
        }
        if self.max_attempts == 0 {
            return Err("field `max_attempts` must be greater than zero".to_owned());
        }

        validate_names(&self.required_inputs, "required_inputs", InputName::new)?;
        validate_names(&self.optional_inputs, "optional_inputs", InputName::new)?;
        validate_names(&self.outputs, "outputs", OutputName::new)?;
        reject_overlap(
            &self.required_inputs,
            &self.optional_inputs,
            "required_inputs",
            "optional_inputs",
        )?;

        let participant_ids = self
            .participants
            .iter()
            .map(|participant| {
                RoleId::new(&participant.role_id)
                    .map_err(|error| error.to_string())
                    .map(|role_id| role_id.as_str().to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        reject_duplicates(&participant_ids, "participants.role_id")?;
        let participant_set = participant_ids.iter().cloned().collect::<BTreeSet<_>>();

        let mut stage_ids = Vec::with_capacity(self.stages.len());
        for (index, stage) in self.stages.iter().enumerate() {
            let step_id = StepId::new(&stage.id).map_err(|error| error.to_string())?;
            stage_ids.push(step_id.as_str().to_owned());
            RoleId::new(&stage.owner_role_id).map_err(|error| error.to_string())?;
            if !participant_set.contains(stage.owner_role_id.trim()) {
                return Err(format!(
                    "stage `{}` names unknown owner role `{}`",
                    stage.id, stage.owner_role_id
                ));
            }
            require_non_blank(
                &stage.instructions,
                &format!("stages[{index}].instructions"),
            )?;
            StepHandlerKind::new(&stage.handler).map_err(|error| error.to_string())?;
            if stage.num_agents == 0 {
                return Err(format!(
                    "stage `{}` must request at least one agent",
                    stage.id
                ));
            }
            if stage.review_rounds > 0 && stage.num_agents < 2 {
                return Err(format!(
                    "stage `{}` requests review rounds with fewer than two agents",
                    stage.id
                ));
            }
        }
        reject_duplicates(&stage_ids, "stages.id")?;
        if stage_ids
            .iter()
            .any(|id| id.eq_ignore_ascii_case("completed"))
        {
            return Err("stage id `completed` is reserved for the terminal state".to_owned());
        }

        let generated_triggers = stage_ids
            .iter()
            .map(|id| format!("{id}_completed"))
            .collect::<BTreeSet<_>>();
        for stage_id in &stage_ids {
            if generated_triggers.contains(stage_id)
                || matches!(
                    stage_id.as_str(),
                    "request_intervention" | "respond_to_intervention"
                )
            {
                return Err(format!(
                    "stage id `{stage_id}` collides with a generated transition or role capability"
                ));
            }
        }

        if let Some(approval) = &self.final_approval {
            RoleId::new(&approval.role_id).map_err(|error| error.to_string())?;
            if !participant_set.contains(approval.role_id.trim()) {
                return Err(format!(
                    "final approval names unknown role `{}`",
                    approval.role_id
                ));
            }
            GuardName::new(&approval.guard_name).map_err(|error| error.to_string())?;
            TransitionTrigger::new(&approval.trigger).map_err(|error| error.to_string())?;
            let generated_guards = stage_ids
                .iter()
                .map(|id| format!("{id}_completed"))
                .collect::<BTreeSet<_>>();
            if generated_guards.contains(approval.guard_name.trim()) {
                return Err(format!(
                    "final approval guard `{}` collides with a generated completion guard",
                    approval.guard_name
                ));
            }
            if generated_triggers.contains(approval.trigger.trim())
                || stage_ids.iter().any(|id| id == approval.trigger.trim())
                || matches!(
                    approval.trigger.trim(),
                    "request_intervention" | "respond_to_intervention"
                )
            {
                return Err(format!(
                    "final approval trigger `{}` collides with a stage, generated transition or role capability",
                    approval.trigger
                ));
            }
        }

        for participant in &self.participants {
            let role_id = participant.role_id.trim();
            let owns_stage = self
                .stages
                .iter()
                .any(|stage| stage.owner_role_id.trim() == role_id);
            let owns_approval = self
                .final_approval
                .as_ref()
                .is_some_and(|approval| approval.role_id.trim() == role_id);
            if participant.capabilities.is_empty() && !owns_stage && !owns_approval {
                return Err(format!(
                    "participant role `{role_id}` has no stage, approval or intervention capability"
                ));
            }
        }

        Ok(())
    }

    #[allow(clippy::too_many_lines)] // The linear topology is rendered atomically from one intent.
    fn into_document(self) -> CeremonyDocument {
        let participant_order = self
            .participants
            .iter()
            .map(|participant| participant.role_id.trim().to_owned())
            .collect::<Vec<_>>();
        let mut actions = self
            .participants
            .iter()
            .map(|participant| {
                let capabilities = participant
                    .capabilities
                    .iter()
                    .map(|capability| capability.as_action().to_owned())
                    .collect::<BTreeSet<_>>();
                (participant.role_id.trim().to_owned(), capabilities)
            })
            .collect::<BTreeMap<_, _>>();

        let stage_state_ids = self
            .stages
            .iter()
            .map(|stage| stage.id.trim().to_ascii_uppercase())
            .collect::<Vec<_>>();
        let mut states = self
            .stages
            .iter()
            .enumerate()
            .map(|(index, _)| StateDocument {
                id: stage_state_ids[index].clone(),
                initial: index == 0,
                terminal: false,
            })
            .collect::<Vec<_>>();
        states.push(StateDocument {
            id: "COMPLETED".to_owned(),
            initial: false,
            terminal: true,
        });

        let mut guards = BTreeMap::new();
        let mut transitions = Vec::with_capacity(self.stages.len());
        let mut steps = Vec::with_capacity(self.stages.len());
        for (index, stage) in self.stages.iter().enumerate() {
            let stage_id = stage.id.trim();
            let completion_guard = format!("{stage_id}_completed");
            guards.insert(
                completion_guard.clone(),
                GuardDocument {
                    guard_type: "automated".to_owned(),
                    check: format!("step_status:{stage_id}:COMPLETED"),
                },
            );

            let is_last = index + 1 == self.stages.len();
            let (trigger, transition_guards, transition_owner) = if is_last {
                if let Some(approval) = &self.final_approval {
                    guards.insert(
                        approval.guard_name.trim().to_owned(),
                        GuardDocument {
                            guard_type: "human".to_owned(),
                            check: "manual_approval".to_owned(),
                        },
                    );
                    (
                        approval.trigger.trim().to_owned(),
                        vec![completion_guard, approval.guard_name.trim().to_owned()],
                        approval.role_id.trim().to_owned(),
                    )
                } else {
                    (
                        format!("{stage_id}_completed"),
                        vec![completion_guard],
                        stage.owner_role_id.trim().to_owned(),
                    )
                }
            } else {
                (
                    format!("{stage_id}_completed"),
                    vec![completion_guard],
                    stage.owner_role_id.trim().to_owned(),
                )
            };

            actions
                .get_mut(stage.owner_role_id.trim())
                .expect("owner role validated")
                .insert(stage_id.to_owned());
            actions
                .get_mut(&transition_owner)
                .expect("transition role validated")
                .insert(trigger.clone());

            transitions.push(TransitionDocument {
                from: stage_state_ids[index].clone(),
                to: stage_state_ids
                    .get(index + 1)
                    .cloned()
                    .unwrap_or_else(|| "COMPLETED".to_owned()),
                trigger,
                guards: transition_guards,
            });
            steps.push(StepDocument {
                id: stage_id.to_owned(),
                state: stage_state_ids[index].clone(),
                handler: stage.handler.trim().to_owned(),
                config: stage_config(stage, index),
            });
        }

        let roles = participant_order
            .into_iter()
            .map(|role_id| RoleDocument {
                allowed_actions: actions
                    .remove(&role_id)
                    .expect("participant action bucket exists")
                    .into_iter()
                    .collect(),
                id: role_id,
            })
            .collect();

        CeremonyDocument {
            version: self.version.trim().to_owned(),
            name: self.name.trim().to_owned(),
            description: self.objective.trim().to_owned(),
            inputs: InputsDocument {
                required: trimmed(self.required_inputs),
                optional: trimmed(self.optional_inputs),
            },
            outputs: self
                .outputs
                .into_iter()
                .map(|output| (output.trim().to_owned(), json!({ "type": "object" })))
                .collect(),
            states,
            transitions,
            steps,
            guards,
            roles,
            timeouts: TimeoutsDocument {
                step_default: self.step_timeout_seconds,
            },
            retry_policies: RetryPoliciesDocument {
                default: RetryPolicyDocument {
                    max_attempts: self.max_attempts,
                    backoff_seconds: self.backoff_seconds,
                },
            },
        }
    }
}

fn stage_config(stage: &StageIntent, index: usize) -> BTreeMap<String, Value> {
    let mut config = BTreeMap::from([
        ("num_agents".to_owned(), json!(stage.num_agents)),
        ("prompt".to_owned(), json!(stage.instructions.trim())),
        (
            "see_prior".to_owned(),
            json!(stage.see_prior.unwrap_or(index > 0)),
        ),
    ]);
    if stage.review_rounds > 0 {
        config.insert("rounds".to_owned(), json!(stage.review_rounds));
    }
    config
}

fn require_non_blank(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("field `{field}` must not be blank"))
    } else {
        Ok(())
    }
}

fn validate_names<T, E>(
    values: &[String],
    field: &str,
    constructor: impl Fn(String) -> Result<T, E>,
) -> Result<(), String>
where
    E: std::fmt::Display,
{
    for value in values {
        constructor(value.clone()).map_err(|error| error.to_string())?;
    }
    let normalized = values
        .iter()
        .map(|value| value.trim().to_owned())
        .collect::<Vec<_>>();
    reject_duplicates(&normalized, field)
}

fn reject_duplicates(values: &[String], field: &str) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(format!("field `{field}` contains duplicate `{value}`"));
        }
    }
    Ok(())
}

fn reject_overlap(
    left: &[String],
    right: &[String],
    left_name: &str,
    right_name: &str,
) -> Result<(), String> {
    let left = left
        .iter()
        .map(|value| value.trim())
        .collect::<BTreeSet<_>>();
    if let Some(overlap) = right
        .iter()
        .map(|value| value.trim())
        .find(|value| left.contains(value))
    {
        return Err(format!(
            "`{overlap}` appears in both `{left_name}` and `{right_name}`"
        ));
    }
    Ok(())
}

fn trimmed(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .collect()
}

const fn default_step_timeout_seconds() -> u64 {
    300
}

const fn default_max_attempts() -> u32 {
    2
}

const fn default_backoff_seconds() -> u64 {
    1
}

fn default_version() -> String {
    "1.0".to_owned()
}

fn default_handler() -> String {
    "host_callback".to_owned()
}

const fn default_num_agents() -> u64 {
    1
}

fn default_approval_guard() -> String {
    "human_approved_outcome".to_owned()
}

fn default_approval_trigger() -> String {
    "approve_outcome".to_owned()
}

#[derive(Debug, Serialize)]
struct CeremonyDocument {
    version: String,
    name: String,
    description: String,
    inputs: InputsDocument,
    outputs: BTreeMap<String, Value>,
    states: Vec<StateDocument>,
    transitions: Vec<TransitionDocument>,
    steps: Vec<StepDocument>,
    guards: BTreeMap<String, GuardDocument>,
    roles: Vec<RoleDocument>,
    timeouts: TimeoutsDocument,
    retry_policies: RetryPoliciesDocument,
}

#[derive(Debug, Serialize)]
struct InputsDocument {
    required: Vec<String>,
    optional: Vec<String>,
}

#[derive(Debug, Serialize)]
struct StateDocument {
    id: String,
    #[serde(skip_serializing_if = "is_false")]
    initial: bool,
    #[serde(skip_serializing_if = "is_false")]
    terminal: bool,
}

#[derive(Debug, Serialize)]
struct TransitionDocument {
    from: String,
    to: String,
    trigger: String,
    guards: Vec<String>,
}

#[derive(Debug, Serialize)]
struct StepDocument {
    id: String,
    state: String,
    handler: String,
    config: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
struct GuardDocument {
    #[serde(rename = "type")]
    guard_type: String,
    check: String,
}

#[derive(Debug, Serialize)]
struct RoleDocument {
    id: String,
    allowed_actions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TimeoutsDocument {
    step_default: u64,
}

#[derive(Debug, Serialize)]
struct RetryPoliciesDocument {
    default: RetryPolicyDocument,
}

#[derive(Debug, Serialize)]
struct RetryPolicyDocument {
    max_attempts: u32,
    backoff_seconds: u64,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip predicate receives a reference.
const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn designs_a_publishable_draft_with_a_real_human_guard() {
        let request = EmbeddedDesignCeremonyRequest::try_from(&intent()).unwrap();
        let designed = request.design().unwrap();
        let report = designed.draft().analyze();

        assert!(report.is_valid(), "{:?}", report.findings());
        assert!(designed.definition_yaml().contains("type: human"));
        assert!(designed
            .definition_yaml()
            .contains("check: manual_approval"));
        assert!(designed.definition_yaml().contains("rounds: 1"));
        assert!(designed.final_approval_required());
    }

    #[test]
    fn refuses_a_stage_owned_by_nobody_at_the_table() {
        let mut value = intent();
        value["stages"][0]["owner_role_id"] = json!("MISSING");

        let error = EmbeddedDesignCeremonyRequest::try_from(&value)
            .unwrap()
            .design()
            .unwrap_err();

        assert!(error.contains("unknown owner role"), "{error}");
    }

    #[test]
    fn refuses_review_rounds_that_would_silently_do_nothing() {
        let mut value = intent();
        value["stages"][1]["num_agents"] = json!(1);

        let error = EmbeddedDesignCeremonyRequest::try_from(&value)
            .unwrap()
            .design()
            .unwrap_err();

        assert!(error.contains("fewer than two agents"), "{error}");
    }

    #[test]
    fn refuses_names_that_would_make_role_actions_ambiguous() {
        let mut value = intent();
        value["stages"][0]["id"] = json!("request_intervention");

        let error = EmbeddedDesignCeremonyRequest::try_from(&value)
            .unwrap()
            .design()
            .unwrap_err();

        assert!(error.contains("role capability"), "{error}");
    }

    #[test]
    fn refuses_a_participant_with_no_possible_action() {
        let mut value = intent();
        value["participants"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "role_id": "OBSERVER" }));

        let error = EmbeddedDesignCeremonyRequest::try_from(&value)
            .unwrap()
            .design()
            .unwrap_err();

        assert!(error.contains("has no stage"), "{error}");
    }
}
