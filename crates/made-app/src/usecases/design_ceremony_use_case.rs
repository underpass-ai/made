//! [`DesignCeremonyUseCase`] — turn authoring intent into a ceremony.
//!
//! The author says what the working session is for, who sits at it and
//! what each stage asks; this decides the mechanical half — one state
//! and one automated completion guard per stage, the transitions
//! between them, the optional human gate at the end, the role actions,
//! the retry policy — and builds the definition they can publish.
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
//! draft goes through the same analysis as every hand-authored one, which is what makes
//! "designed" mean nothing more than "written quickly".

use std::collections::{BTreeMap, BTreeSet};

use made_core::error::DomainError;
use serde_json::{json, Value};

use super::ceremony_design_document::CeremonyDesignDocument;
use super::ceremony_design_stage::CeremonyDesignStage;
use super::designed_ceremony::DesignedCeremony;

use made_core::entities::CeremonyDefinitionDraft;
use made_core::value_objects::{
    Attributes, CeremonyGuard, CeremonyInputDefinition, CeremonyOutputDefinition, CeremonyRole,
    CeremonyState, CeremonyStep, CeremonyTransition, CeremonyVersion, DurationMs, GuardCondition,
    GuardName, RepeatUntilCondition, RetryPolicy, RoleAction, StateId, StepAttempt,
    StepHandlerConfig, StepHandlerKind, StepRepeatPolicy, StepStatus, StepTimeout,
    TransitionTrigger,
};

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

    /// Refuse an intent that cannot become a ceremony, then build the
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

        Ok(DesignedCeremony::new(build_definition(document)?))
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

/// The linear topology, assembled atomically from one intent: one state
/// per stage plus the terminal one, one automated completion guard per
/// stage, one transition out of each, and the final approval's human
/// guard where the author asked for it.
#[allow(clippy::too_many_lines)] // The linear topology is assembled atomically from one intent.
fn build_definition(
    document: &CeremonyDesignDocument,
) -> Result<CeremonyDefinitionDraft, DomainError> {
    let mut actions = document
        .participants()
        .iter()
        .map(|participant| {
            let capabilities = participant
                .capabilities()
                .iter()
                .map(|capability| {
                    RoleAction::from_capability_label(capability.as_action())
                        .expect("known capability")
                })
                .collect::<BTreeSet<_>>();
            (participant.role_id().clone(), capabilities)
        })
        .collect::<BTreeMap<_, _>>();
    let state_ids = document
        .stages()
        .iter()
        .map(|stage| StateId::new(stage.id().as_str().to_ascii_uppercase()))
        .collect::<Result<Vec<_>, _>>()?;
    let terminal = StateId::new(COMPLETED_STATE)?;
    let mut states = state_ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            if index == 0 {
                CeremonyState::initial(id.clone())
            } else {
                CeremonyState::intermediate(id.clone())
            }
        })
        .collect::<Vec<_>>();
    states.push(CeremonyState::terminal(terminal.clone()));
    let retry = RetryPolicy::new(
        StepAttempt::new(document.max_attempts().unwrap_or(DEFAULT_MAX_ATTEMPTS))?,
        DurationMs::from_millis(
            document
                .backoff_seconds()
                .unwrap_or(DEFAULT_BACKOFF_SECONDS)
                .saturating_mul(1000),
        ),
    );
    let timeout = StepTimeout::new(DurationMs::from_millis(
        document
            .step_timeout_seconds()
            .unwrap_or(DEFAULT_STEP_TIMEOUT_SECONDS)
            .saturating_mul(1000),
    ))?;
    let mut guards = Vec::new();
    let mut transitions = Vec::new();
    let mut steps = Vec::new();
    for (index, stage) in document.stages().iter().enumerate() {
        let completion = GuardName::new(completion_guard(stage.id().as_str()))?;
        guards.push(CeremonyGuard::new(
            completion.clone(),
            GuardCondition::StepStatus {
                step_id: stage.id().clone(),
                status: StepStatus::Completed,
            },
        ));
        let (trigger, required_guards, owner) = match document.final_approval() {
            Some(approval) if index + 1 == document.stages().len() => {
                let human = GuardName::new(approval_guard_name(document))?;
                guards.push(CeremonyGuard::new(
                    human.clone(),
                    GuardCondition::HumanApproval,
                ));
                (
                    TransitionTrigger::new(approval_trigger(document))?,
                    vec![completion, human],
                    approval.role_id(),
                )
            }
            _ => (
                TransitionTrigger::new(completion.as_str())?,
                vec![completion],
                stage.owner_role_id(),
            ),
        };
        actions
            .get_mut(stage.owner_role_id())
            .expect("validated owner")
            .insert(RoleAction::step(stage.id().clone()));
        actions
            .get_mut(owner)
            .expect("validated transition owner")
            .insert(RoleAction::transition(trigger.clone()));
        transitions.push(CeremonyTransition::new(
            state_ids[index].clone(),
            state_ids.get(index + 1).unwrap_or(&terminal).clone(),
            trigger,
            required_guards,
        )?);
        let handler = stage
            .handler()
            .cloned()
            .unwrap_or(StepHandlerKind::new(DEFAULT_HANDLER)?);
        let mut step = CeremonyStep::new(
            stage.id().clone(),
            state_ids[index].clone(),
            handler,
            StepHandlerConfig::new(Attributes::new(stage_config(stage, index))?),
            retry,
            Some(timeout),
        );
        if let Some(repeat) = stage.repeat() {
            step = step.with_repeat_policy(StepRepeatPolicy::new(
                RepeatUntilCondition::output_field_equals(
                    repeat.output_field().clone(),
                    repeat.equals().clone(),
                ),
                repeat.max_iterations(),
            ));
        }
        steps.push(step);
    }
    let roles = document
        .participants()
        .iter()
        .map(|participant| {
            CeremonyRole::new(
                participant.role_id().clone(),
                actions
                    .remove(participant.role_id())
                    .expect("validated role"),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inputs = document
        .required_inputs()
        .iter()
        .cloned()
        .map(CeremonyInputDefinition::required)
        .chain(
            document
                .optional_inputs()
                .iter()
                .cloned()
                .map(CeremonyInputDefinition::optional),
        );
    Ok(CeremonyDefinitionDraft::new(
        document.name().clone(),
        document
            .version()
            .cloned()
            .unwrap_or(CeremonyVersion::new(DEFAULT_VERSION)?),
        Some(document.objective().clone()),
        inputs,
        document
            .outputs()
            .iter()
            .cloned()
            .map(CeremonyOutputDefinition::new),
        states,
        transitions,
        steps,
        guards,
        roles,
    ))
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
        let draft = designed.definition();
        assert_eq!(draft.version().as_str(), "1.0");
        assert_eq!(draft.name().as_str(), "art_review");
        assert_eq!(draft.states().len(), 3);
        assert!(draft.states()[0].is_initial());
        assert_eq!(draft.states()[2].id().as_str(), "COMPLETED");
        assert!(draft.states()[2].is_terminal());
        assert_eq!(draft.transitions()[1].trigger().as_str(), "approve_outcome");
        assert!(draft
            .guards()
            .iter()
            .any(|guard| guard.name().as_str() == "human_approved_outcome"
                && matches!(guard.condition(), GuardCondition::HumanApproval)));
        assert!(draft.guards().iter().any(|guard| guard.name().as_str() == "compose_completed"
            && matches!(guard.condition(), GuardCondition::StepStatus { step_id, status: StepStatus::Completed } if step_id.as_str() == "compose")));
        assert_eq!(
            draft.steps()[0]
                .handler_config()
                .attributes()
                .get("see_prior"),
            Some(&json!(false))
        );
        assert_eq!(
            draft.steps()[1]
                .handler_config()
                .attributes()
                .get("see_prior"),
            Some(&json!(true))
        );
        assert_eq!(
            draft.steps()[1].handler_config().attributes().get("rounds"),
            Some(&json!(1))
        );
        assert_eq!(
            draft.steps()[0].timeout().unwrap().duration().get(),
            300_000
        );
        assert_eq!(draft.steps()[0].retry_policy().max_attempts().get(), 2);

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

        let designed = designed(&document);
        let repeat = designed.definition().steps()[0].repeat_policy().unwrap();
        assert_eq!(repeat.max_iterations().get(), 5);
        assert!(
            matches!(repeat.until(), RepeatUntilCondition::OutputFieldEquals { field, expected }
            if field.as_str() == "ready" && expected == &json!(true))
        );
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

        let designed = designed(&document);
        let draft = designed.definition();
        assert_eq!(draft.version().as_str(), "1.0");
        let step = &draft.steps()[0];
        assert_eq!(step.handler_kind().as_str(), "host_callback");
        assert_eq!(
            step.handler_config().attributes().get("num_agents"),
            Some(&json!(1))
        );
        assert_eq!(step.timeout().unwrap().duration().get(), 300_000);
        assert_eq!(step.retry_policy().backoff().get(), 1_000);
    }
}
