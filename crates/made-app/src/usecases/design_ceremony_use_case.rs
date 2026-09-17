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

use made_core::error::DomainError;

use super::ceremony_design_document::CeremonyDesignDocument;
use super::ceremony_design_stage::CeremonyDesignStage;
use super::designed_ceremony::DesignedCeremony;

mod definition;
mod pattern;
mod validation;

use definition::build_definition;
use pattern::materialize;
use validation::validate;

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
        let document = materialize(document)?;
        validate(&document)?;

        Ok(DesignedCeremony::new(build_definition(&document)?))
    }
}

fn num_agents(stage: &CeremonyDesignStage) -> u64 {
    stage
        .num_agents()
        .map_or(DEFAULT_NUM_AGENTS, |agents| u64::from(agents.get()))
}

fn completion_guard(stage_id: &str) -> String {
    format!("{stage_id}_completed")
}

fn exit_guard_name(stage_id: &str, index: usize) -> String {
    format!("exit_{stage_id}_{}", index + 1)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usecases::ceremony_design_final_approval::CeremonyDesignFinalApproval;
    use crate::usecases::ceremony_design_participant::CeremonyDesignParticipant;
    use crate::usecases::ceremony_design_repeat::CeremonyDesignRepeat;
    use crate::usecases::ceremony_participant_capability::CeremonyParticipantCapability;
    use crate::usecases::CeremonyPatternPreset;
    use made_core::value_objects::{
        CeremonyDescription, CeremonyName, InputName, NumAgents, OutputName, RoleId, Rounds,
        StepId, StepInstructions, StepIteration, StepOutputField,
    };
    use made_core::value_objects::{GuardCondition, RepeatUntilCondition, StepStatus};
    use serde_json::json;

    fn role(id: &str) -> RoleId {
        RoleId::new(id).expect("a role id")
    }

    fn instructions(value: impl Into<String>) -> StepInstructions {
        StepInstructions::new(value).expect("stage instructions")
    }

    fn agents(value: u32) -> NumAgents {
        NumAgents::new(value).expect("an agent count")
    }

    fn rounds(value: u32) -> Rounds {
        Rounds::new(value).expect("review rounds")
    }

    fn stage(id: &str, owner: &str) -> CeremonyDesignStage {
        CeremonyDesignStage::new(
            StepId::new(id).expect("a step id"),
            role(owner),
            instructions(format!("Do the {id} work.")),
            None,
            None,
            None,
            rounds(0),
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
                    instructions("Review the candidate."),
                    None,
                    None,
                    Some(agents(2)),
                    rounds(1),
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
                instructions("Compose the candidate."),
                None,
                None,
                None,
                rounds(0),
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
                instructions("Review the candidate."),
                None,
                None,
                Some(agents(1)),
                rounds(1),
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

    #[test]
    fn roundtable_preset_gives_each_participant_one_ordered_turn() {
        let base = document();
        let intent = CeremonyDesignDocument::new(
            base.name().clone(),
            None,
            base.objective().clone(),
            base.required_inputs().to_vec(),
            Vec::new(),
            base.outputs().to_vec(),
            base.participants().to_vec(),
            Vec::new(),
            None,
            None,
            None,
            None,
        )
        .with_pattern(CeremonyPatternPreset::RoundtableFixedOrder);

        let designed = designed(&intent);
        let draft = designed.definition();
        assert_eq!(draft.steps().len(), 2);
        assert_eq!(draft.steps()[0].id().as_str(), "roundtable_turn_1");
        assert_eq!(draft.steps()[1].id().as_str(), "roundtable_turn_2");
        assert_eq!(draft.roles()[0].id().as_str(), "WORKER");
        assert_eq!(draft.roles()[1].id().as_str(), "ARTIST");
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
    }

    #[test]
    fn a_pattern_refuses_explicit_stages_or_a_one_person_roundtable() {
        let base = document();
        let both = base
            .clone()
            .with_pattern(CeremonyPatternPreset::RoundtableFixedOrder);
        assert!(
            refused(&both).contains("mutually exclusive"),
            "pattern plus stages must be refused"
        );

        let one_person = CeremonyDesignDocument::new(
            base.name().clone(),
            None,
            base.objective().clone(),
            Vec::new(),
            Vec::new(),
            base.outputs().to_vec(),
            vec![base.participants()[0].clone()],
            Vec::new(),
            None,
            None,
            None,
            None,
        )
        .with_pattern(CeremonyPatternPreset::RoundtableFixedOrder);
        assert!(
            refused(&one_person).contains("at least two participants"),
            "a roundtable needs more than one role"
        );
    }
}
