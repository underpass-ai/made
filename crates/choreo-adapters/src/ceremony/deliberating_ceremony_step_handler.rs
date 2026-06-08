use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use async_trait::async_trait;
use choreo_app::usecases::DeliberateUseCase;
use choreo_core::entities::{Task, TaskConstraints};
use choreo_core::error::DomainError;
use choreo_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use choreo_core::value_objects::{Attributes, StepOutput, StepResult, TaskDescription, TaskId};
use serde_json::{json, Value};
use tracing::info;

use super::DeliberationStepConfig;

/// Attribute key under which a deliberation winner's content is stored.
const WINNER_CONTENT_KEY: &str = "winner_content";
/// Defensive per-turn cap when rendering a prior contribution into the
/// prompt, so one runaway turn cannot dominate the brief.
const MAX_RENDERED_CONTRIBUTION_LEN: usize = 4000;

#[derive(Debug, Clone)]
pub struct DeliberatingCeremonyStepHandler {
    deliberate: Arc<DeliberateUseCase>,
}

impl DeliberatingCeremonyStepHandler {
    #[must_use]
    pub fn new(deliberate: Arc<DeliberateUseCase>) -> Self {
        Self { deliberate }
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for DeliberatingCeremonyStepHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        let config = DeliberationStepConfig::from_request(&request)?;
        let task = task_from(&request, &config)?;
        let output = self.deliberate.execute(task).await?;
        let ranked = output.deliberation.ranked_outcomes()?;
        let winner = ranked
            .iter()
            .find(|candidate| candidate.proposal().id() == &output.winner_proposal_id)
            .ok_or(DomainError::InvariantViolated {
                reason: "ceremony step winner proposal is missing from ranked outcomes",
            })?;

        info!(
            ceremony_id = request.instance_id().as_str(),
            step_id = request.step_id().as_str(),
            specialty = config.specialty().as_str(),
            winner_proposal_id = winner.proposal().id().as_str(),
            "ceremony step deliberation completed"
        );

        StepResult::completed(StepOutput::new(Attributes::new(output_attributes(
            &request,
            &output.winner_proposal_id,
            winner.proposal().content(),
            ranked.len(),
        ))?))
    }
}

fn task_from(
    request: &CeremonyStepHandlerRequest,
    config: &DeliberationStepConfig,
) -> Result<Task, DomainError> {
    Ok(Task::new(
        task_id_from(request)?,
        config.specialty().clone(),
        build_description(request, config)?,
        TaskConstraints::new(
            choreo_core::value_objects::Rubric::empty(),
            config.rounds(),
            config.num_agents(),
            None,
        ),
        Attributes::new(task_attributes(request, config))?,
    ))
}

/// Build the brief an agent deliberates on: the ceremony role and meeting
/// frame, the prior interventions rendered as prose (when the step opts
/// into the transcript via `see_prior`), and finally the step's own
/// instruction. Framing it as a natural, role-aware brief — rather than a
/// raw JSON context bundle — is what lets the agents speak in character
/// and answer what came before.
fn build_description(
    request: &CeremonyStepHandlerRequest,
    config: &DeliberationStepConfig,
) -> Result<TaskDescription, DomainError> {
    let mut text = String::new();
    let ceremony = request.definition_name().as_str();
    let stage = request.current_state().as_str();

    match request.role_id() {
        Some(role) => {
            let _ = writeln!(
                text,
                "You are acting as {role} in the \"{ceremony}\" ceremony (current stage: {stage}).",
                role = role.as_str(),
            );
        }
        None => {
            let _ = writeln!(
                text,
                "You are a participant in the \"{ceremony}\" ceremony (current stage: {stage}).",
            );
        }
    }

    if config.see_prior() && !request.transcript().is_empty() {
        text.push_str("\nThe meeting so far:\n");
        for contribution in request.transcript().contributions() {
            let said = contribution
                .output()
                .attributes()
                .get(WINNER_CONTENT_KEY)
                .and_then(Value::as_str)
                .unwrap_or("(no content recorded)");
            let _ = writeln!(
                text,
                "- {role} ({step}): {said}",
                role = contribution.role_id().as_str(),
                step = contribution.step_id().as_str(),
                said = truncate(said, MAX_RENDERED_CONTRIBUTION_LEN),
            );
        }
    }

    let _ = write!(
        text,
        "\nYour task now: {}",
        config.task_description().as_str()
    );

    TaskDescription::new(text)
}

/// Cap a rendered prior turn so a single runaway contribution cannot
/// dominate the brief; appends an ellipsis when truncated.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn task_id_from(request: &CeremonyStepHandlerRequest) -> Result<TaskId, DomainError> {
    TaskId::new(format!(
        "ceremony-{}-{}-{}",
        request.instance_id().as_str(),
        request.step_id().as_str(),
        request.attempt().get()
    ))
}

fn task_attributes(
    request: &CeremonyStepHandlerRequest,
    config: &DeliberationStepConfig,
) -> BTreeMap<String, Value> {
    let mut attributes = request.context().attributes().as_map().clone();
    attributes.insert(
        "ceremony.instance_id".to_owned(),
        json!(request.instance_id().as_str()),
    );
    attributes.insert(
        "ceremony.definition_name".to_owned(),
        json!(request.definition_name().as_str()),
    );
    attributes.insert(
        "ceremony.definition_version".to_owned(),
        json!(request.definition_version().as_str()),
    );
    attributes.insert(
        "ceremony.current_state".to_owned(),
        json!(request.current_state().as_str()),
    );
    attributes.insert(
        "ceremony.step_id".to_owned(),
        json!(request.step_id().as_str()),
    );
    attributes.insert(
        "ceremony.handler_kind".to_owned(),
        json!(request.handler_kind().as_str()),
    );
    attributes.insert(
        "ceremony.deliberation_specialty".to_owned(),
        json!(config.specialty().as_str()),
    );
    attributes.insert(
        "ceremony.attempt".to_owned(),
        json!(request.attempt().get()),
    );
    attributes
}

fn output_attributes(
    request: &CeremonyStepHandlerRequest,
    winner_proposal_id: &choreo_core::value_objects::ProposalId,
    winner_content: &str,
    candidates_total: usize,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "task_id".to_owned(),
            json!(format!(
                "ceremony-{}-{}-{}",
                request.instance_id().as_str(),
                request.step_id().as_str(),
                request.attempt().get()
            )),
        ),
        (
            "winner_proposal_id".to_owned(),
            json!(winner_proposal_id.as_str()),
        ),
        ("winner_content".to_owned(), json!(winner_content)),
        (
            "candidates_total".to_owned(),
            json!(u64::try_from(candidates_total).unwrap_or(u64::MAX)),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use choreo_app::usecases::DeliberateUseCase;
    use choreo_core::entities::Council;
    use choreo_core::ports::{ClockPort, CouncilRegistryPort, StatisticsPort};
    use choreo_core::value_objects::{
        AgentId, Attributes, CeremonyContext, CeremonyId, CeremonyName, CeremonyStepContribution,
        CeremonyTranscript, CeremonyVersion, CouncilId, RoleId, Specialty, StateId, StepAttempt,
        StepHandlerConfig, StepHandlerKind, StepId, StepOutput, StepStatus,
    };
    use serde_json::json;

    use crate::clock::SystemClock;
    use crate::memory::{
        InMemoryAgentRegistry, InMemoryCouncilRegistry, InMemoryDeliberationRepository,
        InMemoryStatistics,
    };
    use crate::noop::{NoopAgent, NoopMessaging};
    use crate::scoring::UniformScoring;
    use crate::validators::ContentNonEmptyValidator;

    use super::*;

    #[tokio::test]
    async fn executes_step_by_running_deliberation_for_configured_specialty() {
        let specialty = Specialty::new("facilitation_prompt").unwrap();
        let agent_id = AgentId::new("agent-facilitation_prompt-0").unwrap();
        let agent_registry = Arc::new(InMemoryAgentRegistry::new());
        agent_registry
            .insert(Arc::new(NoopAgent::new(
                agent_id.clone(),
                specialty.clone(),
            )))
            .await
            .unwrap();

        let council_registry = Arc::new(InMemoryCouncilRegistry::new());
        council_registry
            .register(
                Council::new(
                    CouncilId::new("council-facilitation_prompt").unwrap(),
                    specialty.clone(),
                    vec![agent_id],
                    SystemClock::new().now(),
                )
                .unwrap(),
            )
            .await
            .unwrap();

        let statistics = Arc::new(InMemoryStatistics::new());
        let deliberate = Arc::new(DeliberateUseCase::new(
            Arc::new(SystemClock::new()),
            council_registry,
            agent_registry,
            vec![Arc::new(ContentNonEmptyValidator::new())],
            Arc::new(UniformScoring::new()),
            Arc::new(InMemoryDeliberationRepository::new()),
            Arc::new(NoopMessaging::new()),
            statistics.clone(),
            "ceremony-step-handler-test",
        ));
        let handler = DeliberatingCeremonyStepHandler::new(deliberate);

        let result = handler.execute(request_with_prompt()).await.unwrap();

        assert_eq!(result.status(), StepStatus::Completed);
        let attributes = result.output().attributes();
        assert_eq!(
            attributes.get("task_id").and_then(|value| value.as_str()),
            Some("ceremony-ceremony-1-open_room-1")
        );
        assert!(attributes
            .get("winner_proposal_id")
            .and_then(|value| value.as_str())
            .is_some());
        assert!(attributes
            .get("winner_content")
            .and_then(|value| value.as_str())
            .is_some_and(|content| content.contains("Open the meeting")));
        assert_eq!(
            attributes
                .get("candidates_total")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            statistics
                .snapshot()
                .await
                .unwrap()
                .per_specialty()
                .get(&specialty)
                .copied(),
            Some(1)
        );
    }

    #[test]
    fn description_without_role_or_transcript_is_well_formed() {
        let request = request_with_prompt();
        let config = DeliberationStepConfig::from_request(&request).unwrap();

        let text = build_description(&request, &config).unwrap();
        let text = text.as_str();

        assert!(text.contains("participant in the \"editorial\" ceremony"));
        assert!(!text.contains("The meeting so far"));
        assert!(text.contains("Your task now: Open the meeting"));
    }

    #[test]
    fn description_frames_the_role_and_renders_prior_turns_as_prose() {
        let contribution = CeremonyStepContribution::new(
            StepId::new("open_room").unwrap(),
            RoleId::new("FACILITATOR").unwrap(),
            StepOutput::new(
                Attributes::new(BTreeMap::from([(
                    "winner_content".to_owned(),
                    json!("Restating the brief and inviting perspectives."),
                )]))
                .unwrap(),
            ),
        );
        let request = request_with_prompt()
            .with_role(RoleId::new("CUSTOMER_ADVOCATE").unwrap())
            .with_transcript(CeremonyTranscript::empty().appended(contribution));
        let config = DeliberationStepConfig::from_request(&request).unwrap();

        let text = build_description(&request, &config).unwrap();
        let text = text.as_str();

        assert!(text.contains("acting as CUSTOMER_ADVOCATE in the \"editorial\" ceremony"));
        assert!(text.contains("The meeting so far:"));
        assert!(text
            .contains("FACILITATOR (open_room): Restating the brief and inviting perspectives."));
        assert!(text.contains("Your task now: Open the meeting"));
    }

    fn request_with_prompt() -> CeremonyStepHandlerRequest {
        CeremonyStepHandlerRequest::new(
            CeremonyId::new("ceremony-1").unwrap(),
            CeremonyName::new("editorial").unwrap(),
            CeremonyVersion::v1(),
            StateId::new("OPENING").unwrap(),
            StepId::new("open_room").unwrap(),
            StepHandlerKind::new("facilitation_prompt").unwrap(),
            StepHandlerConfig::new(
                Attributes::new(BTreeMap::from([
                    ("prompt".to_owned(), json!("Open the meeting")),
                    ("num_agents".to_owned(), json!(1)),
                ]))
                .unwrap(),
            ),
            CeremonyContext::empty(),
            StepAttempt::FIRST,
        )
    }
}
