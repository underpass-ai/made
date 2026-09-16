//! [`DesignCeremonyUseCase`] — turn authoring intent into a ceremony.
//!
//! The author says what the working session is for, who sits at it and
//! what each stage asks; this decides the mechanical half — one state
//! and one automated completion guard per stage, the transitions
//! between them, the optional human gate at the end, the role actions,
//! the retry policy — and renders the document they can publish.
//!
//! It lives here rather than in a delivery adapter because it is the
//! whole content of the answer: two adapters deciding it separately is
//! two designers, and an author asking the same question of a cluster
//! and of their own process would get two different ceremonies. The
//! adapters map their own request shape onto
//! [`CeremonyDesignDocument`] and render [`DesignedCeremony`]; neither
//! decides anything.
//!
//! What it deliberately does not do is analyse the result. A designed
//! draft goes through the same parser and the same analysis as every
//! hand-authored one, at the same boundary, which is what makes
//! "designed" mean nothing more than "written quickly".

use std::collections::{BTreeMap, BTreeSet};

use made_core::error::DomainError;
use serde_json::{json, Value};

use super::ceremony_design_document::CeremonyDesignDocument;
use super::ceremony_design_stage::CeremonyDesignStage;
use super::designed_ceremony::DesignedCeremony;

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

/// The terminal state every designed ceremony ends in.
const COMPLETED_STATE: &str = "COMPLETED";
/// Role actions the live agenda reserves; a stage or a trigger that
/// spells one of them would make a role's permissions ambiguous.
const RESERVED_ACTIONS: [&str; 2] = ["request_intervention", "respond_to_intervention"];

/// What an omitted field means. Said once, so that an author leaving
/// the same field out gets the same ceremony on every surface.
const DEFAULT_VERSION: &str = "1.0";
const DEFAULT_HANDLER: &str = "host_callback";
const DEFAULT_NUM_AGENTS: u64 = 1;
const DEFAULT_STEP_TIMEOUT_SECONDS: u64 = 300;
const DEFAULT_MAX_ATTEMPTS: u32 = 2;
const DEFAULT_BACKOFF_SECONDS: u64 = 1;
const DEFAULT_APPROVAL_GUARD: &str = "human_approved_outcome";
const DEFAULT_APPROVAL_TRIGGER: &str = "approve_outcome";

#[derive(Debug, Default, Clone, Copy)]
pub struct DesignCeremonyUseCase;

impl DesignCeremonyUseCase {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Refuse an intent that cannot become a ceremony, then render the
    /// one it describes.
    ///
    /// Everything refused here is a defect no single field is guilty
    /// of — a stage owned by nobody at the table, a name that collides
    /// with a generated one, a participant with nothing to do. The
    /// fields themselves were already checked by the value objects the
    /// caller built.
    #[tracing::instrument(
        name = "design_ceremony",
        skip_all,
        fields(ceremony = %document.name())
    )]
    pub fn execute(
        &self,
        document: &CeremonyDesignDocument,
    ) -> Result<DesignedCeremony, DomainError> {
        validate(document)?;

        let stage_count = document.stages().len();
        let participant_count = document.participants().len();
        let final_approval_required = document.final_approval().is_some();
        let rendered = render(document);
        let definition_yaml =
            serde_yaml::to_string(&rendered).map_err(|_| DomainError::InvalidDocument {
                reason: "ceremony draft could not be rendered as YAML".to_owned(),
            })?;

        Ok(DesignedCeremony::new(
            definition_yaml,
            stage_count,
            participant_count,
            final_approval_required,
        ))
    }
}

fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}

#[allow(clippy::too_many_lines)] // Every input invariant is audited in one authoring gate.
fn validate(document: &CeremonyDesignDocument) -> Result<(), DomainError> {
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
    if document.step_timeout_seconds() == Some(0) {
        return Err(invalid(
            "field `step_timeout_seconds` must be greater than zero",
        ));
    }
    if document.max_attempts() == Some(0) {
        return Err(invalid("field `max_attempts` must be greater than zero"));
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
    for (index, stage) in document.stages().iter().enumerate() {
        stage_ids.push(stage.id().as_str().to_owned());
        if !participant_set.contains(stage.owner_role_id().as_str()) {
            return Err(invalid(format!(
                "stage `{}` names unknown owner role `{}`",
                stage.id(),
                stage.owner_role_id()
            )));
        }
        if stage.instructions().trim().is_empty() {
            return Err(invalid(format!(
                "field `stages[{index}].instructions` must not be blank"
            )));
        }
        if stage.num_agents() == Some(0) {
            return Err(invalid(format!(
                "stage `{}` must request at least one agent",
                stage.id()
            )));
        }
        if stage.review_rounds() > 0 && num_agents(stage) < 2 {
            return Err(invalid(format!(
                "stage `{}` requests review rounds with fewer than two agents",
                stage.id()
            )));
        }
    }
    reject_duplicates(stage_ids.iter().cloned(), "stages.id")?;
    if stage_ids
        .iter()
        .any(|id| id.eq_ignore_ascii_case(COMPLETED_STATE))
    {
        return Err(invalid(
            "stage id `completed` is reserved for the terminal state",
        ));
    }

    let generated_triggers = completion_guards(&stage_ids);
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
        if generated_triggers.contains(&guard_name) {
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

/// The linear topology, rendered atomically from one intent: one state
/// per stage plus the terminal one, one automated completion guard per
/// stage, one transition out of each, and the final approval's human
/// guard where the author asked for it.
#[allow(clippy::too_many_lines)] // The linear topology is rendered atomically from one intent.
fn render(document: &CeremonyDesignDocument) -> CeremonyDocument {
    let mut actions = document
        .participants()
        .iter()
        .map(|participant| {
            let capabilities = participant
                .capabilities()
                .iter()
                .map(|capability| capability.as_action().to_owned())
                .collect::<BTreeSet<_>>();
            (participant.role_id().as_str().to_owned(), capabilities)
        })
        .collect::<BTreeMap<_, _>>();

    let stage_state_ids = document
        .stages()
        .iter()
        .map(|stage| stage.id().as_str().to_ascii_uppercase())
        .collect::<Vec<_>>();
    let mut states = stage_state_ids
        .iter()
        .enumerate()
        .map(|(index, id)| StateDocument {
            id: id.clone(),
            initial: index == 0,
            terminal: false,
        })
        .collect::<Vec<_>>();
    states.push(StateDocument {
        id: COMPLETED_STATE.to_owned(),
        initial: false,
        terminal: true,
    });

    let mut guards = BTreeMap::new();
    let mut transitions = Vec::with_capacity(document.stages().len());
    let mut steps = Vec::with_capacity(document.stages().len());
    for (index, stage) in document.stages().iter().enumerate() {
        let stage_id = stage.id().as_str();
        let completion_guard = completion_guard(stage_id);
        guards.insert(
            completion_guard.clone(),
            GuardDocument {
                guard_type: "automated".to_owned(),
                check: format!("step_status:{stage_id}:COMPLETED"),
            },
        );

        let is_last = index + 1 == document.stages().len();
        let (trigger, transition_guards, transition_owner) = match document.final_approval() {
            // The human gate is the last transition, and it is the
            // approver's to pull: guarded by the stage's completion
            // and by the approval itself.
            Some(approval) if is_last => {
                let guard_name = approval_guard_name(document);
                guards.insert(
                    guard_name.clone(),
                    GuardDocument {
                        guard_type: "human".to_owned(),
                        check: "manual_approval".to_owned(),
                    },
                );
                (
                    approval_trigger(document),
                    vec![completion_guard, guard_name],
                    approval.role_id().as_str().to_owned(),
                )
            }
            _ => (
                completion_guard.clone(),
                vec![completion_guard],
                stage.owner_role_id().as_str().to_owned(),
            ),
        };

        actions
            .get_mut(stage.owner_role_id().as_str())
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
                .unwrap_or_else(|| COMPLETED_STATE.to_owned()),
            trigger,
            guards: transition_guards,
        });
        steps.push(StepDocument {
            id: stage_id.to_owned(),
            state: stage_state_ids[index].clone(),
            handler: stage.handler().map_or_else(
                || DEFAULT_HANDLER.to_owned(),
                |kind| kind.as_str().to_owned(),
            ),
            config: stage_config(stage, index),
            repeat: stage.repeat().map(|repeat| StepRepeatDocument {
                max_iterations: repeat.max_iterations().get(),
                until: RepeatUntilDocument {
                    output_field: repeat.output_field().as_str().to_owned(),
                    equals: repeat.equals().clone(),
                },
            }),
        });
    }

    let roles = document
        .participants()
        .iter()
        .map(|participant| {
            let role_id = participant.role_id().as_str().to_owned();
            RoleDocument {
                allowed_actions: actions
                    .remove(&role_id)
                    .expect("participant action bucket exists")
                    .into_iter()
                    .collect(),
                id: role_id,
            }
        })
        .collect();

    CeremonyDocument {
        version: document.version().map_or_else(
            || DEFAULT_VERSION.to_owned(),
            |version| version.as_str().to_owned(),
        ),
        name: document.name().as_str().to_owned(),
        description: document.objective().as_str().to_owned(),
        inputs: InputsDocument {
            required: document
                .required_inputs()
                .iter()
                .map(|input| input.as_str().to_owned())
                .collect(),
            optional: document
                .optional_inputs()
                .iter()
                .map(|input| input.as_str().to_owned())
                .collect(),
        },
        outputs: document
            .outputs()
            .iter()
            .map(|output| (output.as_str().to_owned(), json!({ "type": "object" })))
            .collect(),
        states,
        transitions,
        steps,
        guards,
        roles,
        timeouts: TimeoutsDocument {
            step_default: document
                .step_timeout_seconds()
                .unwrap_or(DEFAULT_STEP_TIMEOUT_SECONDS),
        },
        retry_policies: RetryPoliciesDocument {
            default: RetryPolicyDocument {
                max_attempts: document.max_attempts().unwrap_or(DEFAULT_MAX_ATTEMPTS),
                backoff_seconds: document
                    .backoff_seconds()
                    .unwrap_or(DEFAULT_BACKOFF_SECONDS),
            },
        },
    }
}

fn stage_config(stage: &CeremonyDesignStage, index: usize) -> BTreeMap<String, Value> {
    let mut config = BTreeMap::from([
        ("num_agents".to_owned(), json!(num_agents(stage))),
        ("prompt".to_owned(), json!(stage.instructions().trim())),
        (
            // Earlier stages are context by default for everything
            // after the first, which has nothing to see.
            "see_prior".to_owned(),
            json!(stage.see_prior().unwrap_or(index > 0)),
        ),
    ]);
    if stage.review_rounds() > 0 {
        config.insert("rounds".to_owned(), json!(stage.review_rounds()));
    }
    config
}

fn num_agents(stage: &CeremonyDesignStage) -> u64 {
    stage.num_agents().unwrap_or(DEFAULT_NUM_AGENTS)
}

fn completion_guard(stage_id: &str) -> String {
    format!("{stage_id}_completed")
}

/// Every name this design generates on its own. A stage, a guard or a
/// trigger the author names must not be one of them, because the
/// generated one would silently win.
fn completion_guards(stage_ids: &[String]) -> BTreeSet<String> {
    stage_ids
        .iter()
        .map(|id| completion_guard(id))
        .collect::<BTreeSet<_>>()
}

fn approval_guard_name(document: &CeremonyDesignDocument) -> String {
    document.final_approval().map_or_else(
        || DEFAULT_APPROVAL_GUARD.to_owned(),
        |approval| {
            approval.guard_name().map_or_else(
                || DEFAULT_APPROVAL_GUARD.to_owned(),
                |name| name.as_str().to_owned(),
            )
        },
    )
}

fn approval_trigger(document: &CeremonyDesignDocument) -> String {
    document.final_approval().map_or_else(
        || DEFAULT_APPROVAL_TRIGGER.to_owned(),
        |approval| {
            approval.trigger().map_or_else(
                || DEFAULT_APPROVAL_TRIGGER.to_owned(),
                |trigger| trigger.as_str().to_owned(),
            )
        },
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usecases::ceremony_design_final_approval::CeremonyDesignFinalApproval;
    use crate::usecases::ceremony_design_participant::CeremonyDesignParticipant;
    use crate::usecases::ceremony_design_repeat::CeremonyDesignRepeat;
    use crate::usecases::ceremony_participant_capability::CeremonyParticipantCapability;
    use made_core::value_objects::{
        CeremonyDescription, CeremonyName, InputName, OutputName, RoleId, StepId, StepIteration,
        StepOutputField,
    };

    fn role(id: &str) -> RoleId {
        RoleId::new(id).expect("a role id")
    }

    fn stage(id: &str, owner: &str) -> CeremonyDesignStage {
        CeremonyDesignStage::new(
            StepId::new(id).expect("a step id"),
            role(owner),
            format!("Do the {id} work."),
            None,
            None,
            None,
            0,
            None,
        )
    }

    /// One session with two stages and a human who accepts the
    /// outcome: compose, review, approve.
    fn document() -> CeremonyDesignDocument {
        CeremonyDesignDocument::new(
            CeremonyName::new("art_review").expect("a ceremony name"),
            None,
            CeremonyDescription::new("Compose one candidate and ask the artist to accept it.")
                .expect("an objective"),
            vec![
                InputName::new("brief").expect("an input"),
                InputName::new("parts").expect("an input"),
            ],
            Vec::new(),
            vec![OutputName::new("candidate_review").expect("an output")],
            vec![
                CeremonyDesignParticipant::new(
                    role("WORKER"),
                    [CeremonyParticipantCapability::RespondToIntervention],
                ),
                CeremonyDesignParticipant::new(
                    role("ARTIST"),
                    [CeremonyParticipantCapability::RequestIntervention],
                ),
            ],
            vec![
                stage("compose", "WORKER"),
                CeremonyDesignStage::new(
                    StepId::new("review").expect("a step id"),
                    role("ARTIST"),
                    "Review the candidate.",
                    None,
                    None,
                    Some(2),
                    1,
                    None,
                ),
            ],
            Some(CeremonyDesignFinalApproval::new(role("ARTIST"), None, None)),
            None,
            None,
            None,
        )
    }

    fn designed(document: &CeremonyDesignDocument) -> DesignedCeremony {
        DesignCeremonyUseCase::new()
            .execute(document)
            .expect("the intent should design a ceremony")
    }

    fn refused(document: &CeremonyDesignDocument) -> String {
        match DesignCeremonyUseCase::new().execute(document) {
            Err(DomainError::InvalidDocument { reason }) => reason,
            other => panic!("the intent should have been refused: {other:?}"),
        }
    }

    /// The whole topology in one pass: the states, the generated
    /// guards and triggers, the human gate and what each seat may do.
    #[test]
    fn one_intent_becomes_one_linear_ceremony() {
        let designed = designed(&document());
        let rendered: serde_yaml::Value =
            serde_yaml::from_str(designed.definition_yaml()).expect("the draft is YAML");

        assert_eq!(rendered["version"], serde_yaml::Value::from("1.0"));
        assert_eq!(rendered["name"], serde_yaml::Value::from("art_review"));
        assert_eq!(
            rendered["states"].as_sequence().map(Vec::len),
            Some(3),
            "one state per stage plus the terminal one"
        );
        assert_eq!(
            rendered["states"][0]["initial"],
            serde_yaml::Value::from(true)
        );
        assert_eq!(
            rendered["states"][2]["id"],
            serde_yaml::Value::from("COMPLETED")
        );
        assert_eq!(
            rendered["states"][2]["terminal"],
            serde_yaml::Value::from(true)
        );
        // The last transition is the human gate: the approver pulls
        // it, and it waits on the stage's completion and on them.
        assert_eq!(
            rendered["transitions"][1]["trigger"],
            serde_yaml::Value::from("approve_outcome")
        );
        assert_eq!(
            rendered["guards"]["human_approved_outcome"]["type"],
            serde_yaml::Value::from("human")
        );
        assert_eq!(
            rendered["guards"]["compose_completed"]["check"],
            serde_yaml::Value::from("step_status:compose:COMPLETED")
        );
        assert_eq!(
            rendered["steps"][0]["config"]["see_prior"],
            serde_yaml::Value::from(false),
            "the first stage has nothing to see"
        );
        assert_eq!(
            rendered["steps"][1]["config"]["see_prior"],
            serde_yaml::Value::from(true)
        );
        assert_eq!(
            rendered["steps"][1]["config"]["rounds"],
            serde_yaml::Value::from(1)
        );
        assert_eq!(
            rendered["timeouts"]["step_default"],
            serde_yaml::Value::from(300)
        );
        assert_eq!(
            rendered["retry_policies"]["default"]["max_attempts"],
            serde_yaml::Value::from(2)
        );

        assert_eq!(designed.topology(), "linear");
        assert_eq!(designed.stage_count(), 2);
        assert_eq!(designed.participant_count(), 2);
        assert!(designed.final_approval_required());
    }

    #[test]
    fn a_bounded_repeating_stage_carries_its_cap_and_its_test() {
        let base = document();
        let stages = vec![
            CeremonyDesignStage::new(
                StepId::new("compose").expect("a step id"),
                role("WORKER"),
                "Compose the candidate.",
                None,
                None,
                None,
                0,
                Some(CeremonyDesignRepeat::new(
                    StepIteration::new(5).expect("an iteration cap"),
                    StepOutputField::new("ready").expect("an output field"),
                    json!(true),
                )),
            ),
            base.stages()[1].clone(),
        ];
        let document = CeremonyDesignDocument::new(
            base.name().clone(),
            None,
            base.objective().clone(),
            base.required_inputs().to_vec(),
            Vec::new(),
            base.outputs().to_vec(),
            base.participants().to_vec(),
            stages,
            base.final_approval().cloned(),
            None,
            None,
            None,
        );

        let yaml = designed(&document).definition_yaml().to_owned();

        assert!(yaml.contains("max_iterations: 5"), "{yaml}");
        assert!(yaml.contains("output_field: ready"), "{yaml}");
    }

    /// One case per rule that no single field is guilty of breaking.
    /// Each one names the element at fault, because an author reading
    /// "a stage is wrong" has to find which.
    #[test]
    fn an_intent_that_cannot_become_a_ceremony_says_which_part() {
        let base = document();

        let with_stages = |stages: Vec<CeremonyDesignStage>| {
            CeremonyDesignDocument::new(
                base.name().clone(),
                None,
                base.objective().clone(),
                base.required_inputs().to_vec(),
                Vec::new(),
                base.outputs().to_vec(),
                base.participants().to_vec(),
                stages,
                base.final_approval().cloned(),
                None,
                None,
                None,
            )
        };

        let orphan = refused(&with_stages(vec![
            stage("compose", "MISSING"),
            base.stages()[1].clone(),
        ]));
        assert!(orphan.contains("unknown owner role"), "{orphan}");
        assert!(orphan.contains("MISSING"), "{orphan}");

        let lonely_review = refused(&with_stages(vec![
            stage("compose", "WORKER"),
            CeremonyDesignStage::new(
                StepId::new("review").expect("a step id"),
                role("ARTIST"),
                "Review the candidate.",
                None,
                None,
                Some(1),
                1,
                None,
            ),
        ]));
        assert!(
            lonely_review.contains("fewer than two agents"),
            "{lonely_review}"
        );

        let ambiguous = refused(&with_stages(vec![
            stage("request_intervention", "WORKER"),
            base.stages()[1].clone(),
        ]));
        assert!(ambiguous.contains("role capability"), "{ambiguous}");

        let reserved = refused(&with_stages(vec![
            stage("completed", "WORKER"),
            base.stages()[1].clone(),
        ]));
        assert!(reserved.contains("reserved"), "{reserved}");

        let mut participants = base.participants().to_vec();
        participants.push(CeremonyDesignParticipant::new(role("OBSERVER"), []));
        let idle = refused(&CeremonyDesignDocument::new(
            base.name().clone(),
            None,
            base.objective().clone(),
            base.required_inputs().to_vec(),
            Vec::new(),
            base.outputs().to_vec(),
            participants,
            base.stages().to_vec(),
            base.final_approval().cloned(),
            None,
            None,
            None,
        ));
        assert!(idle.contains("has no stage"), "{idle}");
        assert!(idle.contains("OBSERVER"), "{idle}");
    }

    /// An omission means one thing, decided here, so the same document
    /// designs the same ceremony whichever surface took it.
    #[test]
    fn what_an_omitted_field_means_is_decided_once() {
        let base = document();
        let document = CeremonyDesignDocument::new(
            base.name().clone(),
            None,
            base.objective().clone(),
            Vec::new(),
            Vec::new(),
            base.outputs().to_vec(),
            base.participants().to_vec(),
            vec![stage("compose", "WORKER")],
            None,
            None,
            None,
            None,
        );

        let rendered: serde_yaml::Value =
            serde_yaml::from_str(designed(&document).definition_yaml()).expect("the draft is YAML");

        assert_eq!(rendered["version"], serde_yaml::Value::from("1.0"));
        assert_eq!(
            rendered["steps"][0]["handler"],
            serde_yaml::Value::from("host_callback")
        );
        assert_eq!(
            rendered["steps"][0]["config"]["num_agents"],
            serde_yaml::Value::from(1)
        );
        assert_eq!(
            rendered["timeouts"]["step_default"],
            serde_yaml::Value::from(300)
        );
        assert_eq!(
            rendered["retry_policies"]["default"]["backoff_seconds"],
            serde_yaml::Value::from(1)
        );
        // No approval asked for, so the last transition is the stage's
        // own completion and no human guard exists to wait on.
        assert_eq!(
            rendered["transitions"][0]["trigger"],
            serde_yaml::Value::from("compose_completed")
        );
        assert!(rendered["guards"]["human_approved_outcome"].is_null());
    }
}
