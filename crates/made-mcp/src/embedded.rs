//! Embedded MCP backend.

mod domain_tool_error;
mod embedded_apply_ceremony_transition_request;
mod embedded_approve_ceremony_guard_request;
mod embedded_assert_ceremony_reason_request;
mod embedded_bind_ceremony_participants_request;
mod embedded_ceremony_draft_presenter;
mod embedded_ceremony_draft_request;
mod embedded_ceremony_history_presenter;
mod embedded_ceremony_id_request;
mod embedded_ceremony_instance_presenter;
mod embedded_claim_ceremony_step_request;
mod embedded_close_ceremony_intervention_request;
mod embedded_collect_ceremony_evidence_request;
mod embedded_complete_ceremony_step_request;
mod embedded_defer_ceremony_guard_request;
mod embedded_design_ceremony_request;
mod embedded_diff_ceremony_definitions_request;
mod embedded_generate_ceremony_report_request;
mod embedded_get_status_request;
mod embedded_publication_presenter;
mod embedded_publish_ceremony_definition_request;
mod embedded_read_ceremony_events_request;
mod embedded_request_ceremony_intervention_request;
mod embedded_request_fields;
mod embedded_respond_to_ceremony_intervention_request;
mod embedded_run_ceremony_presenter;
mod embedded_run_ceremony_request;
mod embedded_run_ceremony_step_request;
mod embedded_service_observability_presenter;
mod embedded_start_ceremony_request;
mod embedded_start_published_ceremony_request;

use made_app::services::CeremonyTraceScope;
use made_app::usecases::CeremonyDraftView;
use made_core::value_objects::{CeremonyId, TraceContext};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::backend::{MadeMcpToolBackend, MadeMcpToolFuture, ToolTraceContext};
use crate::protocol::{
    tool_success_result, ToolError, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL, CLAIM_CEREMONY_STEP_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL, DIFF_CEREMONY_DEFINITIONS_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    GET_CEREMONY_TRANSCRIPT_TOOL, GET_METRICS_TOOL, GET_STATUS_TOOL, LIST_CEREMONY_INSTANCES_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL, READ_CEREMONY_EVENTS_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL, RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL, START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL, VERIFY_CEREMONY_JOURNAL_TOOL,
};
use crate::renderers::{CeremonyInstanceListing, CeremonyInstanceListingEntry};

use self::embedded_apply_ceremony_transition_request::EmbeddedApplyCeremonyTransitionRequest;
use self::embedded_approve_ceremony_guard_request::EmbeddedApproveCeremonyGuardRequest;
use self::embedded_assert_ceremony_reason_request::EmbeddedAssertCeremonyReasonRequest;
use self::embedded_bind_ceremony_participants_request::EmbeddedBindCeremonyParticipantsRequest;
use self::embedded_ceremony_draft_presenter::{
    present_definition_diff, EmbeddedCeremonyDraftPresenter,
};
use self::embedded_ceremony_draft_request::EmbeddedCeremonyDraftRequest;
use self::embedded_ceremony_history_presenter::{
    present_ceremony_events, present_ceremony_journal_verdict, present_ceremony_report,
    present_ceremony_transcript,
};
use self::embedded_ceremony_id_request::EmbeddedCeremonyIdRequest;
use self::embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter;
use self::embedded_claim_ceremony_step_request::EmbeddedClaimCeremonyStepRequest;
use self::embedded_close_ceremony_intervention_request::EmbeddedCloseCeremonyInterventionRequest;
use self::embedded_collect_ceremony_evidence_request::EmbeddedCollectCeremonyEvidenceRequest;
use self::embedded_complete_ceremony_step_request::EmbeddedCompleteCeremonyStepRequest;
use self::embedded_defer_ceremony_guard_request::EmbeddedDeferCeremonyGuardRequest;
use self::embedded_design_ceremony_request::EmbeddedDesignCeremonyRequest;
use self::embedded_diff_ceremony_definitions_request::EmbeddedDiffCeremonyDefinitionsRequest;
use self::embedded_generate_ceremony_report_request::EmbeddedGenerateCeremonyReportRequest;
use self::embedded_get_status_request::EmbeddedGetStatusRequest;
use self::embedded_publication_presenter::EmbeddedPublicationPresenter;
use self::embedded_publish_ceremony_definition_request::EmbeddedPublishCeremonyDefinitionRequest;
use self::embedded_read_ceremony_events_request::EmbeddedReadCeremonyEventsRequest;
use self::embedded_request_ceremony_intervention_request::EmbeddedRequestCeremonyInterventionRequest;
use self::embedded_respond_to_ceremony_intervention_request::EmbeddedRespondToCeremonyInterventionRequest;
use self::embedded_run_ceremony_presenter::EmbeddedRunCeremonyPresenter;
use self::embedded_run_ceremony_request::EmbeddedRunCeremonyRequest;
use self::embedded_run_ceremony_step_request::EmbeddedRunCeremonyStepRequest;
use self::embedded_service_observability_presenter::{
    present_service_metrics, present_service_status,
};
use self::embedded_start_ceremony_request::EmbeddedStartCeremonyRequest;
use self::embedded_start_published_ceremony_request::EmbeddedStartPublishedCeremonyRequest;

/// What this backend calls itself. Shared with the default lease owner
/// rule, so the id an omitted `lease_owner_id` becomes cannot drift from
/// the name `initialize` advertises.
pub(crate) const EMBEDDED_BACKEND_NAME: &str = "embedded";

/// MCP adapter that executes ceremonies inside the host process.
#[derive(Clone, Debug, Default)]
pub struct EmbeddedMadeMcpBackend {
    made: EmbeddedMade,
}

impl EmbeddedMadeMcpBackend {
    #[must_use]
    pub fn new(made: EmbeddedMade) -> Self {
        Self { made }
    }

    async fn present_instance(&self, ceremony_id: &CeremonyId) -> Result<Value, ToolError> {
        EmbeddedCeremonyInstancePresenter::present(&self.made, ceremony_id)
            .await
            .map(tool_success_result)
    }

    async fn present_instances(&self) -> Result<Value, ToolError> {
        let instances = self.made.instances().await?;
        let mut values = Vec::with_capacity(instances.len());
        for instance in instances {
            // An instance whose definition is not in this store cannot be
            // read back — the published-definition restart boundary. The
            // state is still there, so the listing reports that one entry
            // as unreadable instead of taking every readable ceremony down
            // with it. Asking for it by id still fails loudly.
            //
            // Every entry carries `rehydratable` and `reason`, readable
            // or not, so a caller tests one field rather than inferring
            // readability from a field that is not there.
            match EmbeddedCeremonyInstancePresenter::present(&self.made, instance.id()).await {
                Ok(value) => values.push(CeremonyInstanceListingEntry::rehydratable(value)),
                Err(reason) => values.push(CeremonyInstanceListingEntry::unrehydratable(
                    instance.id().as_str(),
                    reason.message(),
                )),
            }
        }
        Ok(tool_success_result(
            CeremonyInstanceListing::new(values).to_json(),
        ))
    }
}

impl MadeMcpToolBackend for EmbeddedMadeMcpBackend {
    fn backend_name(&self) -> &'static str {
        EMBEDDED_BACKEND_NAME
    }

    fn supports_tool(&self, name: &str) -> bool {
        matches!(
            name,
            RUN_CEREMONY_TOOL
                | START_CEREMONY_TOOL
                | RUN_CEREMONY_STEP_TOOL
                | CLAIM_CEREMONY_STEP_TOOL
                | COMPLETE_CEREMONY_STEP_TOOL
                | APPROVE_CEREMONY_GUARD_TOOL
                | DEFER_CEREMONY_GUARD_TOOL
                | APPLY_CEREMONY_TRANSITION_TOOL
                | GET_CEREMONY_INSTANCE_TOOL
                | LIST_CEREMONY_INSTANCES_TOOL
                | REQUEST_CEREMONY_INTERVENTION_TOOL
                | RESPOND_TO_CEREMONY_INTERVENTION_TOOL
                | CLOSE_CEREMONY_INTERVENTION_TOOL
                | COLLECT_CEREMONY_EVIDENCE_TOOL
                | ASSERT_CEREMONY_REASON_TOOL
                | DESIGN_CEREMONY_TOOL
                | VALIDATE_CEREMONY_DRAFT_TOOL
                | EXPLAIN_CEREMONY_DRAFT_TOOL
                | PUBLISH_CEREMONY_DEFINITION_TOOL
                | START_PUBLISHED_CEREMONY_TOOL
                | DIFF_CEREMONY_DEFINITIONS_TOOL
                | BIND_CEREMONY_PARTICIPANTS_TOOL
                | READ_CEREMONY_EVENTS_TOOL
                | VERIFY_CEREMONY_JOURNAL_TOOL
                | GET_CEREMONY_TRANSCRIPT_TOOL
                | GENERATE_CEREMONY_REPORT_TOOL
                | GET_STATUS_TOOL
                | GET_METRICS_TOOL
        )
    }

    // A dispatch table: one arm per tool, and splitting it would put
    // half the surface somewhere a reader has to go looking for.
    #[allow(clippy::too_many_lines)]
    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> MadeMcpToolFuture<'a> {
        Box::pin(async move {
            match name {
                DESIGN_CEREMONY_TOOL => {
                    let designed = EmbeddedDesignCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?
                        .execute(&self.made)?;
                    let draft = designed.definition();
                    let yaml = made_adapters::yaml::DesignedCeremonyYaml::render(&designed)?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedCeremonyDraftPresenter::present_design(
                            &yaml,
                            &CeremonyDraftView::project(draft, &report),
                            &designed,
                        ),
                    ))
                }
                VALIDATE_CEREMONY_DRAFT_TOOL => {
                    let request = EmbeddedCeremonyDraftRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let draft = request.parse()?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedCeremonyDraftPresenter::present_validation(
                            &CeremonyDraftView::project(&draft, &report),
                        ),
                    ))
                }
                EXPLAIN_CEREMONY_DRAFT_TOOL => {
                    let request = EmbeddedCeremonyDraftRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let draft = request.parse()?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedCeremonyDraftPresenter::present_explanation(
                            &CeremonyDraftView::project(&draft, &report),
                        ),
                    ))
                }
                BIND_CEREMONY_PARTICIPANTS_TOOL => {
                    let request = EmbeddedBindCeremonyParticipantsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let instance = request.execute(&self.made).await?;
                    self.present_instance(instance.id()).await
                }
                DIFF_CEREMONY_DEFINITIONS_TOOL => {
                    let request = EmbeddedDiffCeremonyDefinitionsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let diff = request.execute(&self.made).await?;
                    Ok(tool_success_result(present_definition_diff(&diff)))
                }
                PUBLISH_CEREMONY_DEFINITION_TOOL => {
                    let request = EmbeddedPublishCeremonyDefinitionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let outcome = request.execute(&self.made).await?;
                    Ok(tool_success_result(EmbeddedPublicationPresenter::present(
                        &outcome,
                    )))
                }
                START_PUBLISHED_CEREMONY_TOOL => {
                    let request = EmbeddedStartPublishedCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RUN_CEREMONY_TOOL => {
                    let request = EmbeddedRunCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let output = request.execute(&self.made).await?;
                    Ok(tool_success_result(EmbeddedRunCeremonyPresenter::present(
                        &output,
                    )))
                }
                START_CEREMONY_TOOL => {
                    let request = EmbeddedStartCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RUN_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedRunCeremonyStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                CLAIM_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedClaimCeremonyStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                COMPLETE_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedCompleteCeremonyStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                APPROVE_CEREMONY_GUARD_TOOL => {
                    let request = EmbeddedApproveCeremonyGuardRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                ASSERT_CEREMONY_REASON_TOOL => {
                    let request = EmbeddedAssertCeremonyReasonRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                DEFER_CEREMONY_GUARD_TOOL => {
                    let request = EmbeddedDeferCeremonyGuardRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                APPLY_CEREMONY_TRANSITION_TOOL => {
                    let request = EmbeddedApplyCeremonyTransitionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                GET_CEREMONY_INSTANCE_TOOL => {
                    let request = EmbeddedCeremonyIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.into_ceremony_id();
                    self.present_instance(&ceremony_id).await
                }
                READ_CEREMONY_EVENTS_TOOL => {
                    let request = EmbeddedReadCeremonyEventsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let input = request.into_input().map_err(ToolError::invalid_request)?;
                    let page = self
                        .made
                        .audit_records_from(
                            input.ceremony_id(),
                            input.from_version(),
                            Some(input.limit()),
                        )
                        .await?;
                    present_ceremony_events(&page).map(tool_success_result)
                }
                VERIFY_CEREMONY_JOURNAL_TOOL => {
                    let request = EmbeddedCeremonyIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let verdict = self
                        .made
                        .verify_journal(&request.into_ceremony_id())
                        .await?;
                    Ok(tool_success_result(present_ceremony_journal_verdict(
                        &verdict,
                    )))
                }
                GET_CEREMONY_TRANSCRIPT_TOOL => {
                    let request = EmbeddedCeremonyIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let transcript = self.made.transcript(&request.into_ceremony_id()).await?;
                    Ok(tool_success_result(present_ceremony_transcript(
                        &transcript,
                    )))
                }
                GENERATE_CEREMONY_REPORT_TOOL => {
                    // A mapper, not a composer: the projection is the
                    // use case's (ADR-006), and the server renders the
                    // same one.
                    let input = EmbeddedGenerateCeremonyReportRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?
                        .into_input();
                    let report = self.made.report(input).await?;
                    Ok(tool_success_result(present_ceremony_report(&report)))
                }
                GET_STATUS_TOOL => {
                    // The same use case the deployable edition's
                    // `GetStatus` runs, rendered into the same four
                    // keys: what an engine says about itself is not
                    // allowed to depend on which one a client reached.
                    let request = EmbeddedGetStatusRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let status = self.made.status(request.include_statistics()).await?;
                    Ok(tool_success_result(present_service_status(&status)))
                }
                GET_METRICS_TOOL => {
                    let metrics = self.made.metrics().await?;
                    Ok(tool_success_result(present_service_metrics(&metrics)))
                }
                LIST_CEREMONY_INSTANCES_TOOL => self.present_instances().await,
                REQUEST_CEREMONY_INTERVENTION_TOOL => {
                    let request = EmbeddedRequestCeremonyInterventionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RESPOND_TO_CEREMONY_INTERVENTION_TOOL => {
                    let request = EmbeddedRespondToCeremonyInterventionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                CLOSE_CEREMONY_INTERVENTION_TOOL => {
                    let request = EmbeddedCloseCeremonyInterventionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                COLLECT_CEREMONY_EVIDENCE_TOOL => {
                    let request = EmbeddedCollectCeremonyEvidenceRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                _ => Err(ToolError::invalid_request(format!(
                    "embedded backend: unsupported tool `{name}`"
                ))),
            }
        })
    }

    fn call_tool_with_trace<'a>(
        &'a self,
        name: &'a str,
        arguments: &'a Value,
        trace: &'a ToolTraceContext,
    ) -> MadeMcpToolFuture<'a> {
        Box::pin(async move {
            let trace = TraceContext::parse(trace.traceparent())
                .map_err(|error| ToolError::invalid_request(error.to_string()))?;
            CeremonyTraceScope::run(trace, self.call_tool(name, arguments)).await
        })
    }
}
