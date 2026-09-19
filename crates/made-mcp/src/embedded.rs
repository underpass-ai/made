//! Embedded MCP backend.

mod domain_tool_error;
mod embedded_accept_child_completion_request;
mod embedded_apply_ceremony_transition_request;
mod embedded_approve_ceremony_guard_request;
mod embedded_artifact_dispatch;
mod embedded_assert_ceremony_reason_request;
mod embedded_authorization_dispatch;
mod embedded_authorization_presenter;
mod embedded_authorization_request;
mod embedded_backend_authorization;
mod embedded_bind_ceremony_participants_request;
mod embedded_budget_dispatch;
mod embedded_budget_fields;
mod embedded_cancel_ceremony_request;
mod embedded_ceremony_draft_presenter;
mod embedded_ceremony_draft_request;
mod embedded_ceremony_history_presenter;
mod embedded_ceremony_id_request;
mod embedded_ceremony_instance_presenter;
mod embedded_ceremony_listing;
mod embedded_ceremony_search_request;
mod embedded_children_request;
mod embedded_claim_ceremony_step_request;
mod embedded_close_ceremony_intervention_request;
mod embedded_collect_ceremony_evidence_request;
mod embedded_complete_ceremony_step_request;
mod embedded_council_context;
mod embedded_council_dispatch;
mod embedded_council_journal_dispatch;
mod embedded_council_presenter;
mod embedded_council_requests;
mod embedded_defer_ceremony_guard_request;
mod embedded_deliberation_observer;
mod embedded_design_ceremony_request;
mod embedded_diff_ceremony_definitions_request;
mod embedded_execution_receipt_request;
mod embedded_extension_dispatch;
mod embedded_generate_ceremony_report_request;
mod embedded_get_status_request;
mod embedded_pause_ceremony_request;
mod embedded_publication_presenter;
mod embedded_publish_ceremony_definition_request;
mod embedded_pull_ceremony_events_request;
mod embedded_read_ceremony_events_request;
mod embedded_recover_ceremony_children_request;
mod embedded_request_ceremony_intervention_request;
mod embedded_request_fields;
mod embedded_respond_to_ceremony_intervention_request;
mod embedded_resume_ceremony_request;
mod embedded_run_ceremony_presenter;
mod embedded_run_ceremony_request;
mod embedded_run_ceremony_step_request;
mod embedded_service_observability_presenter;
mod embedded_start_ceremony_request;
mod embedded_start_published_ceremony_request;
mod embedded_stream_ceremony_request;
mod embedded_tool_authorizer;

use made_app::services::{AuthorizationOperationScope, CeremonyTraceScope};
use made_app::usecases::CeremonyDraftView;
use made_core::value_objects::{
    AuthorizationRequestId, CeremonyEventPageLimit, CeremonyId, TraceContext,
};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::backend::{
    MadeMcpBackendInitializationFuture, MadeMcpToolBackend, MadeMcpToolFuture, ToolTraceContext,
};
use crate::protocol::{
    tool_success_result, ToolError, ACCEPT_CHILD_COMPLETION_TOOL, ADOPT_EXECUTION_RECEIPT_TOOL,
    APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL, ASSERT_CEREMONY_REASON_TOOL,
    BIND_CEREMONY_PARTICIPANTS_TOOL, CANCEL_CEREMONY_TOOL, CLAIM_CEREMONY_STEP_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    COMPLETE_EXECUTION_RECEIPT_TOOL, DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL,
    DIFF_CEREMONY_DEFINITIONS_TOOL, ENFORCE_CEREMONY_DEADLINES_TOOL, EXPLAIN_CEREMONY_DRAFT_TOOL,
    GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_INSTANCE_TOOL, GET_CEREMONY_TRANSCRIPT_TOOL,
    GET_EXECUTION_RECEIPT_TOOL, GET_METRICS_TOOL, GET_STATUS_TOOL, INSPECT_EXECUTION_RECOVERY_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, PAUSE_CEREMONY_TOOL, PREPARE_CEREMONY_CHILDREN_TOOL,
    PUBLISH_CEREMONY_DEFINITION_TOOL, PULL_CEREMONY_EVENTS_TOOL, READ_CEREMONY_EVENTS_TOOL,
    RECOVER_CEREMONY_CHILDREN_TOOL, REQUEST_CEREMONY_INTERVENTION_TOOL,
    RESPOND_TO_CEREMONY_INTERVENTION_TOOL, RESUME_CEREMONY_TOOL, RUN_CEREMONY_STEP_TOOL,
    RUN_CEREMONY_TOOL, SEARCH_CEREMONY_INSTANCES_TOOL, START_CEREMONY_TOOL,
    START_PUBLISHED_CEREMONY_TOOL, STREAM_CEREMONY_TOOL, VALIDATE_CEREMONY_DRAFT_TOOL,
    VERIFY_CEREMONY_JOURNAL_TOOL,
};

use self::embedded_accept_child_completion_request::EmbeddedAcceptChildCompletionRequest;
use self::embedded_apply_ceremony_transition_request::EmbeddedApplyCeremonyTransitionRequest;
use self::embedded_approve_ceremony_guard_request::EmbeddedApproveCeremonyGuardRequest;
use self::embedded_assert_ceremony_reason_request::EmbeddedAssertCeremonyReasonRequest;
use self::embedded_bind_ceremony_participants_request::EmbeddedBindCeremonyParticipantsRequest;
use self::embedded_cancel_ceremony_request::EmbeddedCancelCeremonyRequest;
use self::embedded_ceremony_draft_presenter::{
    present_definition_diff, EmbeddedCeremonyDraftPresenter,
};
use self::embedded_ceremony_draft_request::EmbeddedCeremonyDraftRequest;
use self::embedded_ceremony_history_presenter::{
    collect_ceremony_progress, present_ceremony_events, present_ceremony_journal_verdict,
    present_ceremony_report, present_ceremony_transcript, present_pulled_ceremony_events,
};
use self::embedded_ceremony_id_request::EmbeddedCeremonyIdRequest;
use self::embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter;
use self::embedded_ceremony_search_request::EmbeddedCeremonySearchRequest;
use self::embedded_children_request::EmbeddedPrepareCeremonyChildrenRequest;
use self::embedded_claim_ceremony_step_request::EmbeddedClaimCeremonyStepRequest;
use self::embedded_close_ceremony_intervention_request::EmbeddedCloseCeremonyInterventionRequest;
use self::embedded_collect_ceremony_evidence_request::EmbeddedCollectCeremonyEvidenceRequest;
use self::embedded_complete_ceremony_step_request::EmbeddedCompleteCeremonyStepRequest;
use self::embedded_defer_ceremony_guard_request::EmbeddedDeferCeremonyGuardRequest;
use self::embedded_design_ceremony_request::EmbeddedDesignCeremonyRequest;
use self::embedded_diff_ceremony_definitions_request::EmbeddedDiffCeremonyDefinitionsRequest;
use self::embedded_execution_receipt_request::{
    operation_id as execution_operation_id, present_receipt, present_recovery_page, recovery_page,
    EmbeddedApplyExecutionReceiptRequest,
};
use self::embedded_generate_ceremony_report_request::EmbeddedGenerateCeremonyReportRequest;
use self::embedded_get_status_request::EmbeddedGetStatusRequest;
use self::embedded_pause_ceremony_request::EmbeddedPauseCeremonyRequest;
use self::embedded_publication_presenter::EmbeddedPublicationPresenter;
use self::embedded_publish_ceremony_definition_request::EmbeddedPublishCeremonyDefinitionRequest;
use self::embedded_pull_ceremony_events_request::EmbeddedPullCeremonyEventsRequest;
use self::embedded_read_ceremony_events_request::EmbeddedReadCeremonyEventsRequest;
use self::embedded_recover_ceremony_children_request::EmbeddedRecoverCeremonyChildrenRequest;
use self::embedded_request_ceremony_intervention_request::EmbeddedRequestCeremonyInterventionRequest;
use self::embedded_respond_to_ceremony_intervention_request::EmbeddedRespondToCeremonyInterventionRequest;
use self::embedded_resume_ceremony_request::EmbeddedResumeCeremonyRequest;
use self::embedded_run_ceremony_presenter::EmbeddedRunCeremonyPresenter;
use self::embedded_run_ceremony_request::EmbeddedRunCeremonyRequest;
use self::embedded_run_ceremony_step_request::EmbeddedRunCeremonyStepRequest;
use self::embedded_service_observability_presenter::{
    present_service_metrics, present_service_status,
};
use self::embedded_start_ceremony_request::EmbeddedStartCeremonyRequest;
use self::embedded_start_published_ceremony_request::EmbeddedStartPublishedCeremonyRequest;
use self::embedded_stream_ceremony_request::EmbeddedStreamCeremonyRequest;

pub(crate) const EMBEDDED_BACKEND_NAME: &str = "embedded";

/// MCP adapter that executes ceremonies inside the host process.
#[derive(Clone, Debug, Default)]
pub struct EmbeddedMadeMcpBackend {
    made: EmbeddedMade,
    authorization: Option<embedded_tool_authorizer::EmbeddedToolAuthorizer>,
}

impl EmbeddedMadeMcpBackend {
    #[must_use]
    pub fn new(made: EmbeddedMade) -> Self {
        Self {
            made,
            authorization: None,
        }
    }

    async fn present_instance(&self, ceremony_id: &CeremonyId) -> Result<Value, ToolError> {
        EmbeddedCeremonyInstancePresenter::present(&self.made, ceremony_id)
            .await
            .map(tool_success_result)
    }
}

impl MadeMcpToolBackend for EmbeddedMadeMcpBackend {
    fn backend_name(&self) -> &'static str {
        EMBEDDED_BACKEND_NAME
    }

    fn initialize(&self) -> MadeMcpBackendInitializationFuture<'_> {
        Box::pin(async {
            if let Some(authorization) = &self.authorization {
                authorization.validate_configuration().await?;
            }
            self.made
                .recover_event_publication()
                .await
                .map_err(|error| format!("embedded event publication recovery failed: {error}"))?;
            loop {
                let round = self
                    .made
                    .recover_children(CeremonyEventPageLimit::DEFAULT)
                    .await
                    .map_err(|error| format!("embedded child recovery failed: {error}"))?;
                if round.failed > 0 {
                    return Err(format!(
                        "embedded child recovery left {} durable record(s) pending",
                        round.failed
                    ));
                }
                if round.busy || round.acknowledged() == 0 {
                    return Ok(());
                }
            }
        })
    }

    fn supports_tool(&self, name: &str) -> bool {
        if embedded_extension_dispatch::handles(name) {
            return true;
        }
        matches!(
            name,
            RUN_CEREMONY_TOOL
                | START_CEREMONY_TOOL
                | RUN_CEREMONY_STEP_TOOL
                | PREPARE_CEREMONY_CHILDREN_TOOL
                | ACCEPT_CHILD_COMPLETION_TOOL
                | RECOVER_CEREMONY_CHILDREN_TOOL
                | CLAIM_CEREMONY_STEP_TOOL
                | COMPLETE_CEREMONY_STEP_TOOL
                | GET_EXECUTION_RECEIPT_TOOL
                | INSPECT_EXECUTION_RECOVERY_TOOL
                | COMPLETE_EXECUTION_RECEIPT_TOOL
                | ADOPT_EXECUTION_RECEIPT_TOOL
                | APPROVE_CEREMONY_GUARD_TOOL
                | DEFER_CEREMONY_GUARD_TOOL
                | APPLY_CEREMONY_TRANSITION_TOOL
                | PAUSE_CEREMONY_TOOL
                | RESUME_CEREMONY_TOOL
                | CANCEL_CEREMONY_TOOL
                | ENFORCE_CEREMONY_DEADLINES_TOOL
                | GET_CEREMONY_INSTANCE_TOOL
                | LIST_CEREMONY_INSTANCES_TOOL
                | SEARCH_CEREMONY_INSTANCES_TOOL
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
                | STREAM_CEREMONY_TOOL
                | PULL_CEREMONY_EVENTS_TOOL
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
            if let Some(result) =
                embedded_extension_dispatch::dispatch(&self.made, name, arguments).await
            {
                return result;
            }
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
                    let ceremony_id = Box::pin(request.execute(&self.made)).await?;
                    self.present_instance(&ceremony_id).await
                }
                RUN_CEREMONY_TOOL => {
                    let request = EmbeddedRunCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let output = Box::pin(request.execute(&self.made)).await?;
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
                PREPARE_CEREMONY_CHILDREN_TOOL => {
                    let request = EmbeddedPrepareCeremonyChildrenRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    Box::pin(request.execute(&self.made))
                        .await
                        .map(tool_success_result)
                }
                ACCEPT_CHILD_COMPLETION_TOOL => {
                    let request = EmbeddedAcceptChildCompletionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    request.execute(&self.made).await.map(tool_success_result)
                }
                RECOVER_CEREMONY_CHILDREN_TOOL => {
                    let request = EmbeddedRecoverCeremonyChildrenRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    request.execute(&self.made).await.map(tool_success_result)
                }
                CLAIM_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedClaimCeremonyStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let (claim, budget) = request.execute(&self.made).await?;
                    let mut value =
                        EmbeddedCeremonyInstancePresenter::present_claim(&self.made, &claim)
                            .await?;
                    value["budget"] = budget.unwrap_or(Value::Null);
                    Ok(tool_success_result(value))
                }
                COMPLETE_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedCompleteCeremonyStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                GET_EXECUTION_RECEIPT_TOOL => {
                    let operation_id =
                        execution_operation_id(arguments).map_err(ToolError::invalid_request)?;
                    let receipt = self.made.get_execution_receipt(&operation_id).await?;
                    Ok(tool_success_result(present_receipt(&receipt)))
                }
                INSPECT_EXECUTION_RECOVERY_TOOL => {
                    let (after, limit) =
                        recovery_page(arguments).map_err(ToolError::invalid_request)?;
                    let page = self
                        .made
                        .inspect_execution_recovery(after.as_ref(), limit)
                        .await?;
                    Ok(tool_success_result(present_recovery_page(&page)))
                }
                COMPLETE_EXECUTION_RECEIPT_TOOL | ADOPT_EXECUTION_RECEIPT_TOOL => {
                    let request = EmbeddedApplyExecutionReceiptRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let link_kind = if name == COMPLETE_EXECUTION_RECEIPT_TOOL {
                        made_core::value_objects::ExecutionReceiptLinkKind::Direct
                    } else {
                        made_core::value_objects::ExecutionReceiptLinkKind::Adopted
                    };
                    let ceremony_id = request.execute(&self.made, link_kind).await?;
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
                PAUSE_CEREMONY_TOOL => {
                    let request = EmbeddedPauseCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RESUME_CEREMONY_TOOL => {
                    let request = EmbeddedResumeCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                CANCEL_CEREMONY_TOOL => {
                    let request = EmbeddedCancelCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                ENFORCE_CEREMONY_DEADLINES_TOOL => {
                    let request = EmbeddedCeremonyIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let ceremony_id = request.into_ceremony_id();
                    self.made
                        .enforce_ceremony_deadlines(
                            made_app::usecases::EnforceCeremonyDeadlinesInput::new(
                                ceremony_id.clone(),
                            ),
                        )
                        .await?;
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
                            Some(input.limit().value()),
                        )
                        .await?;
                    present_ceremony_events(&page).map(tool_success_result)
                }
                STREAM_CEREMONY_TOOL => {
                    let request = EmbeddedStreamCeremonyRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let input = request.into_input().map_err(ToolError::invalid_request)?;
                    let stream = self.made.stream_ceremony(input).await?;
                    collect_ceremony_progress(stream)
                        .await
                        .map(tool_success_result)
                }
                PULL_CEREMONY_EVENTS_TOOL => {
                    let request = EmbeddedPullCeremonyEventsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let output = self
                        .made
                        .pull_events(request.consumer, request.limit, request.acknowledge_through)
                        .await?;
                    present_pulled_ceremony_events(&output).map(tool_success_result)
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
                LIST_CEREMONY_INSTANCES_TOOL => {
                    embedded_ceremony_listing::present_all(&self.made).await
                }
                SEARCH_CEREMONY_INSTANCES_TOOL => {
                    let request = EmbeddedCeremonySearchRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let request_id = AuthorizationRequestId::new(
                        ToolTraceContext::for_direct_call(name, arguments)
                            .authorization_request_id()
                            .to_owned(),
                    )
                    .map_err(|error| ToolError::invalid_request(error.to_string()))?;
                    request.execute_and_present(&self.made, request_id).await
                }
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
            let tool_trace = trace;
            let authorization_request_id =
                AuthorizationRequestId::new(tool_trace.authorization_request_id().to_owned())
                    .map_err(|error| ToolError::invalid_request(error.to_string()))?;
            let trace = TraceContext::parse(tool_trace.traceparent())
                .map_err(|error| ToolError::invalid_request(error.to_string()))?;
            let dispatch = async {
                let Some(authorization) = &self.authorization else {
                    return self.call_tool(name, arguments).await;
                };
                let operation = authorization
                    .authorize(&self.made, name, arguments, tool_trace)
                    .await?;
                AuthorizationOperationScope::run(operation, async {
                    if name == SEARCH_CEREMONY_INSTANCES_TOOL {
                        return EmbeddedCeremonySearchRequest::try_from(arguments)
                            .map_err(ToolError::invalid_request)?
                            .execute_and_present(&self.made, authorization_request_id)
                            .await;
                    }
                    self.call_tool(name, arguments).await
                })
                .await
            };
            CeremonyTraceScope::run(trace, dispatch).await
        })
    }
}
