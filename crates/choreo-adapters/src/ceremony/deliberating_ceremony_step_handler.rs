use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use choreo_app::usecases::DeliberateUseCase;
use choreo_core::entities::{
    ContextItem, ContextSummary, ExternalContextBundle, Task, TaskConstraints,
};
use choreo_core::error::DomainError;
use choreo_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use choreo_core::value_objects::{Attributes, StepOutput, StepResult, TaskId};
use serde_json::{json, Value};
use tracing::info;

use super::DeliberationStepConfig;

/// `ContextItem.kind` used for prior ceremony interventions.
const CONTRIBUTION_KIND: &str = "ceremony.contribution";
/// Attribute key under which a deliberation winner's content is stored.
const WINNER_CONTENT_KEY: &str = "winner_content";

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
        let external_context = if config.see_prior() {
            external_context_from_transcript(&request)?
        } else {
            None
        };
        let task = task_from(&request, &config, external_context)?;
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
    external_context: Option<ExternalContextBundle>,
) -> Result<Task, DomainError> {
    Ok(Task::new_with_context(
        task_id_from(request)?,
        config.specialty().clone(),
        config.task_description().clone(),
        TaskConstraints::new(
            choreo_core::value_objects::Rubric::empty(),
            config.rounds(),
            config.num_agents(),
            None,
        ),
        Attributes::new(task_attributes(request, config))?,
        external_context,
    ))
}

/// Render the prior-step transcript into an [`ExternalContextBundle`] so
/// the deliberating agents see what earlier steps decided. Returns `None`
/// when the transcript is empty.
fn external_context_from_transcript(
    request: &CeremonyStepHandlerRequest,
) -> Result<Option<ExternalContextBundle>, DomainError> {
    let transcript = request.transcript();
    if transcript.is_empty() {
        return Ok(None);
    }

    let items = transcript
        .contributions()
        .iter()
        .enumerate()
        .map(|(index, contribution)| {
            let narrative = contribution
                .output()
                .attributes()
                .get(WINNER_CONTENT_KEY)
                .and_then(Value::as_str)
                .map(str::to_owned);
            ContextItem::new(
                format!("contribution-{index}"),
                CONTRIBUTION_KIND,
                format!(
                    "{} · {}",
                    contribution.role_id().as_str(),
                    contribution.step_id().as_str()
                ),
                narrative,
                Attributes::empty(),
                Vec::new(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let summary = ContextSummary::new(
        format!(
            "Prior interventions in this ceremony ({} so far).",
            transcript.len()
        ),
        Attributes::empty(),
    )?;

    ExternalContextBundle::new(
        "ceremony-transcript",
        "ceremony.transcript.v1",
        Some(summary),
        items,
        Vec::new(),
        Attributes::empty(),
    )
    .map(Some)
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
    fn empty_transcript_yields_no_external_context() {
        let request = request_with_prompt();

        assert!(external_context_from_transcript(&request)
            .unwrap()
            .is_none());
    }

    #[test]
    fn transcript_renders_each_contribution_as_a_context_item() {
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
            .with_transcript(CeremonyTranscript::empty().appended(contribution));

        let bundle = external_context_from_transcript(&request)
            .unwrap()
            .expect("a non-empty transcript renders a bundle");

        assert_eq!(bundle.items().len(), 1);
        assert_eq!(bundle.items()[0].kind(), "ceremony.contribution");
        assert!(bundle.items()[0].title().contains("FACILITATOR"));
        assert_eq!(
            bundle.items()[0].narrative(),
            Some("Restating the brief and inviting perspectives.")
        );
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
