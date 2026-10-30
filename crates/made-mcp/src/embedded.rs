//! Embedded MCP backend.
mod domain_tool_error;
mod embedded_accept_child_completion_request;
mod embedded_apply_protocol_transition_request;
mod embedded_approve_protocol_guard_request;
mod embedded_assert_protocol_reason_request;
mod embedded_authorization_presenter;
mod embedded_authorization_request;
mod embedded_authorized_dispatch;
mod embedded_backend_authorization;
mod embedded_backend_builder;
mod embedded_backend_presenter;
mod embedded_bind_protocol_participants_request;
mod embedded_budget_fields;
mod embedded_cancel_protocol_request;
mod embedded_children_request;
mod embedded_claim_protocol_step_request;
mod embedded_close_protocol_intervention_request;
mod embedded_collect_protocol_evidence_request;
mod embedded_complete_protocol_step_request;
mod embedded_council_context;
mod embedded_council_presenter;
mod embedded_council_requests;
mod embedded_defer_protocol_guard_request;
mod embedded_deliberation_observer;
mod embedded_design_protocol_request;
mod embedded_diff_protocol_definitions_request;
mod embedded_execution_receipt_request;
mod embedded_extension_dispatch;
mod embedded_generate_protocol_report_request;
mod embedded_get_status_request;
mod embedded_host_handoff_dispatch;
mod embedded_pause_protocol_request;
mod embedded_protocol_draft_presenter;
mod embedded_protocol_draft_request;
mod embedded_protocol_history_presenter;
mod embedded_protocol_id_request;
mod embedded_protocol_instance_presenter;
mod embedded_protocol_search_dispatch;
mod embedded_protocol_search_request;
mod embedded_publication_presenter;
mod embedded_publish_protocol_definition_request;
mod embedded_pull_protocol_events_request;
mod embedded_read_protocol_events_request;
mod embedded_recover_protocol_children_request;
mod embedded_request_fields;
mod embedded_request_protocol_intervention_request;
mod embedded_respond_to_protocol_intervention_request;
mod embedded_resume_protocol_request;
mod embedded_run_protocol_presenter;
mod embedded_run_protocol_request;
mod embedded_run_protocol_step_request;
mod embedded_service_observability_presenter;
mod embedded_start_protocol_request;
mod embedded_start_published_protocol_request;
mod embedded_stream_protocol_request;
mod embedded_tool_authorizer;
mod embedded_tool_support;

use made_app::usecases::ProtocolDraftView;
use made_core::value_objects::{AuthorizationRequestId, ProtocolEventPageLimit};
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::backend::{
    MadeMcpAuthorizationSummaryFuture, MadeMcpBackendInitializationFuture, MadeMcpToolBackend,
    MadeMcpToolFuture, ToolTraceContext,
};
use crate::mcp_protocol::{
    tool_success_result, ToolError, ACCEPT_CHILD_COMPLETION_TOOL, ADOPT_EXECUTION_RECEIPT_TOOL,
    APPLY_PROTOCOL_TRANSITION_TOOL, APPROVE_PROTOCOL_GUARD_TOOL, ASSERT_PROTOCOL_REASON_TOOL,
    BIND_PROTOCOL_PARTICIPANTS_TOOL, CANCEL_PROTOCOL_TOOL, CLAIM_PROTOCOL_STEP_TOOL,
    CLOSE_PROTOCOL_INTERVENTION_TOOL, COLLECT_PROTOCOL_EVIDENCE_TOOL,
    COMPLETE_EXECUTION_RECEIPT_TOOL, COMPLETE_PROTOCOL_STEP_TOOL, DEFER_PROTOCOL_GUARD_TOOL,
    DESIGN_PROTOCOL_TOOL, DIFF_PROTOCOL_DEFINITIONS_TOOL, ENFORCE_PROTOCOL_DEADLINES_TOOL,
    EXPLAIN_PROTOCOL_DRAFT_TOOL, GENERATE_PROTOCOL_REPORT_TOOL, GET_EXECUTION_RECEIPT_TOOL,
    GET_METRICS_TOOL, GET_PROTOCOL_INSTANCE_TOOL, GET_PROTOCOL_TRANSCRIPT_TOOL, GET_STATUS_TOOL,
    INSPECT_EXECUTION_RECOVERY_TOOL, LIST_PROTOCOL_INSTANCES_TOOL, PAUSE_PROTOCOL_TOOL,
    PREPARE_PROTOCOL_CHILDREN_TOOL, PUBLISH_PROTOCOL_DEFINITION_TOOL, PULL_PROTOCOL_EVENTS_TOOL,
    READ_PROTOCOL_EVENTS_TOOL, RECOVER_PROTOCOL_CHILDREN_TOOL, REQUEST_PROTOCOL_INTERVENTION_TOOL,
    RESPOND_TO_PROTOCOL_INTERVENTION_TOOL, RESUME_PROTOCOL_TOOL, RUN_PROTOCOL_STEP_TOOL,
    RUN_PROTOCOL_TOOL, SEARCH_PROTOCOL_INSTANCES_TOOL, START_PROTOCOL_TOOL,
    START_PUBLISHED_PROTOCOL_TOOL, STREAM_PROTOCOL_TOOL, VALIDATE_PROTOCOL_DRAFT_TOOL,
    VERIFY_PROTOCOL_JOURNAL_TOOL,
};

use self::embedded_accept_child_completion_request::EmbeddedAcceptChildCompletionRequest;
use self::embedded_apply_protocol_transition_request::EmbeddedApplyProtocolTransitionRequest;
use self::embedded_approve_protocol_guard_request::EmbeddedApproveProtocolGuardRequest;
use self::embedded_assert_protocol_reason_request::EmbeddedAssertProtocolReasonRequest;
use self::embedded_bind_protocol_participants_request::EmbeddedBindProtocolParticipantsRequest;
use self::embedded_cancel_protocol_request::EmbeddedCancelProtocolRequest;
use self::embedded_children_request::EmbeddedPrepareProtocolChildrenRequest;
use self::embedded_claim_protocol_step_request::EmbeddedClaimProtocolStepRequest;
use self::embedded_close_protocol_intervention_request::EmbeddedCloseProtocolInterventionRequest;
use self::embedded_collect_protocol_evidence_request::EmbeddedCollectProtocolEvidenceRequest;
use self::embedded_complete_protocol_step_request::EmbeddedCompleteProtocolStepRequest;
use self::embedded_protocol_draft_presenter::{
    present_definition_diff, EmbeddedProtocolDraftPresenter,
};
use self::embedded_protocol_draft_request::EmbeddedProtocolDraftRequest;
use self::embedded_protocol_history_presenter::{
    collect_protocol_progress, present_protocol_events, present_protocol_journal_verdict,
    present_protocol_report, present_protocol_transcript, present_pulled_protocol_events,
};
use self::embedded_protocol_id_request::EmbeddedProtocolIdRequest;
use self::embedded_protocol_instance_presenter::EmbeddedProtocolInstancePresenter;
mod embedded_renew_step_lease_request;
use self::embedded_defer_protocol_guard_request::EmbeddedDeferProtocolGuardRequest;
use self::embedded_design_protocol_request::EmbeddedDesignProtocolRequest;
use self::embedded_diff_protocol_definitions_request::EmbeddedDiffProtocolDefinitionsRequest;
use self::embedded_execution_receipt_request::{
    operation_id as execution_operation_id, present_receipt, present_recovery_page, recovery_page,
    EmbeddedApplyExecutionReceiptRequest,
};
use self::embedded_generate_protocol_report_request::EmbeddedGenerateProtocolReportRequest;
use self::embedded_get_status_request::EmbeddedGetStatusRequest;
use self::embedded_pause_protocol_request::EmbeddedPauseProtocolRequest;
use self::embedded_publication_presenter::EmbeddedPublicationPresenter;
use self::embedded_publish_protocol_definition_request::EmbeddedPublishProtocolDefinitionRequest;
use self::embedded_pull_protocol_events_request::EmbeddedPullProtocolEventsRequest;
use self::embedded_read_protocol_events_request::EmbeddedReadProtocolEventsRequest;
use self::embedded_recover_protocol_children_request::EmbeddedRecoverProtocolChildrenRequest;
use self::embedded_request_protocol_intervention_request::EmbeddedRequestProtocolInterventionRequest;
use self::embedded_respond_to_protocol_intervention_request::EmbeddedRespondToProtocolInterventionRequest;
use self::embedded_resume_protocol_request::EmbeddedResumeProtocolRequest;
use self::embedded_run_protocol_presenter::EmbeddedRunProtocolPresenter;
use self::embedded_run_protocol_request::EmbeddedRunProtocolRequest;
use self::embedded_run_protocol_step_request::EmbeddedRunProtocolStepRequest;
use self::embedded_service_observability_presenter::{
    present_service_metrics, present_service_status,
};
use self::embedded_start_protocol_request::EmbeddedStartProtocolRequest;
use self::embedded_start_published_protocol_request::EmbeddedStartPublishedProtocolRequest;
use self::embedded_stream_protocol_request::EmbeddedStreamProtocolRequest;
use crate::human_approval_source::HumanApprovalSource;
use crate::mcp_protocol::RENEW_PROTOCOL_STEP_LEASE_TOOL;

pub(crate) use embedded_tool_support::embedded_supports_tool;

pub(crate) const EMBEDDED_BACKEND_NAME: &str = "embedded";

/// MCP adapter that executes protocols inside the host process.
#[derive(Clone, Debug, Default)]
pub struct EmbeddedMadeMcpBackend {
    made: EmbeddedMade,
    authorization: Option<embedded_tool_authorizer::EmbeddedToolAuthorizer>,
    human_approval_source: HumanApprovalSource,
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
                    .recover_children(ProtocolEventPageLimit::DEFAULT)
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

    fn host_activation_adapter(&self) -> &'static str {
        self.made.host_activation().kind().as_str()
    }

    fn human_approval_source(&self) -> &'static str {
        self.human_approval_source.as_str()
    }

    fn supports_tool(&self, name: &str) -> bool {
        embedded_supports_tool(name)
    }

    fn authorization_summary(&self) -> MadeMcpAuthorizationSummaryFuture<'_> {
        Box::pin(async move {
            let authorization = self.authorization.as_ref()?;
            Some(authorization.authority_summary().await)
        })
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
                DESIGN_PROTOCOL_TOOL => {
                    let designed = EmbeddedDesignProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?
                        .execute(&self.made)?;
                    let draft = designed.definition();
                    let yaml = made_adapters::yaml::DesignedProtocolYaml::render(&designed)?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedProtocolDraftPresenter::present_design(
                            &yaml,
                            &ProtocolDraftView::project(draft, &report),
                            &designed,
                        ),
                    ))
                }
                VALIDATE_PROTOCOL_DRAFT_TOOL => {
                    let request = EmbeddedProtocolDraftRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let draft = request.parse()?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedProtocolDraftPresenter::present_validation(
                            &ProtocolDraftView::project(&draft, &report),
                        ),
                    ))
                }
                EXPLAIN_PROTOCOL_DRAFT_TOOL => {
                    let request = EmbeddedProtocolDraftRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let draft = request.parse()?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedProtocolDraftPresenter::present_explanation(
                            &ProtocolDraftView::project(&draft, &report),
                        ),
                    ))
                }
                BIND_PROTOCOL_PARTICIPANTS_TOOL => {
                    let request = EmbeddedBindProtocolParticipantsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let instance = request.execute(&self.made).await?;
                    self.present_instance(instance.id()).await
                }
                DIFF_PROTOCOL_DEFINITIONS_TOOL => {
                    let request = EmbeddedDiffProtocolDefinitionsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let diff = request.execute(&self.made).await?;
                    Ok(tool_success_result(present_definition_diff(&diff)))
                }
                PUBLISH_PROTOCOL_DEFINITION_TOOL => {
                    let request = EmbeddedPublishProtocolDefinitionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let outcome = request.execute(&self.made).await?;
                    Ok(tool_success_result(EmbeddedPublicationPresenter::present(
                        &outcome,
                    )))
                }
                START_PUBLISHED_PROTOCOL_TOOL => {
                    let request = EmbeddedStartPublishedProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = Box::pin(request.execute(&self.made)).await?;
                    self.present_instance(&protocol_id).await
                }
                RUN_PROTOCOL_TOOL => {
                    let request = EmbeddedRunProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let output = Box::pin(request.execute(&self.made)).await?;
                    Ok(tool_success_result(EmbeddedRunProtocolPresenter::present(
                        &output,
                    )))
                }
                START_PROTOCOL_TOOL => {
                    let request = EmbeddedStartProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                RUN_PROTOCOL_STEP_TOOL => {
                    let request = EmbeddedRunProtocolStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                PREPARE_PROTOCOL_CHILDREN_TOOL => {
                    let request = EmbeddedPrepareProtocolChildrenRequest::try_from(arguments)
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
                RECOVER_PROTOCOL_CHILDREN_TOOL => {
                    let request = EmbeddedRecoverProtocolChildrenRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    request.execute(&self.made).await.map(tool_success_result)
                }
                CLAIM_PROTOCOL_STEP_TOOL => {
                    let request = EmbeddedClaimProtocolStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let (claim, budget) = request.execute(&self.made).await?;
                    let mut value =
                        EmbeddedProtocolInstancePresenter::present_claim(&self.made, &claim)
                            .await?;
                    value["budget"] = budget.unwrap_or(Value::Null);
                    Ok(tool_success_result(value))
                }
                COMPLETE_PROTOCOL_STEP_TOOL => {
                    let request = EmbeddedCompleteProtocolStepRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                RENEW_PROTOCOL_STEP_LEASE_TOOL => {
                    let request = embedded_renew_step_lease_request::parse(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let receipt = self.made.renew_step_lease(request).await?;
                    Ok(tool_success_result(
                        embedded_renew_step_lease_request::present(receipt),
                    ))
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
                    let protocol_id = request.execute(&self.made, link_kind).await?;
                    self.present_instance(&protocol_id).await
                }
                APPROVE_PROTOCOL_GUARD_TOOL => {
                    // In terminal mode the MCP session cannot be the
                    // one deciding, whatever the arguments say.
                    if self.human_approval_source.is_terminal() {
                        return Err(ToolError::refused(self.human_approval_source.refusal()));
                    }
                    let request = EmbeddedApproveProtocolGuardRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                ASSERT_PROTOCOL_REASON_TOOL => {
                    let request = EmbeddedAssertProtocolReasonRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                DEFER_PROTOCOL_GUARD_TOOL => {
                    let request = EmbeddedDeferProtocolGuardRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                APPLY_PROTOCOL_TRANSITION_TOOL => {
                    let request = EmbeddedApplyProtocolTransitionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                PAUSE_PROTOCOL_TOOL => {
                    let request = EmbeddedPauseProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                RESUME_PROTOCOL_TOOL => {
                    let request = EmbeddedResumeProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                crate::mcp_protocol::RECORD_PROTOCOL_HOST_HANDOFF_TOOL => {
                    embedded_host_handoff_dispatch::record(&self.made, arguments).await
                }
                crate::mcp_protocol::INSPECT_PROTOCOL_RESUME_TOOL => {
                    embedded_host_handoff_dispatch::inspect(&self.made, arguments).await
                }
                CANCEL_PROTOCOL_TOOL => {
                    let request = EmbeddedCancelProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                ENFORCE_PROTOCOL_DEADLINES_TOOL => {
                    let request = EmbeddedProtocolIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.into_protocol_id();
                    self.made
                        .enforce_protocol_deadlines(
                            made_app::usecases::EnforceProtocolDeadlinesInput::new(
                                protocol_id.clone(),
                            ),
                        )
                        .await?;
                    self.present_instance(&protocol_id).await
                }
                GET_PROTOCOL_INSTANCE_TOOL => {
                    let request = EmbeddedProtocolIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.into_protocol_id();
                    self.present_instance(&protocol_id).await
                }
                READ_PROTOCOL_EVENTS_TOOL => {
                    let request = EmbeddedReadProtocolEventsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let input = request.into_input().map_err(ToolError::invalid_request)?;
                    let page = self
                        .made
                        .audit_records_from(
                            input.protocol_id(),
                            input.from_version(),
                            Some(input.limit().value()),
                        )
                        .await?;
                    present_protocol_events(&page).map(tool_success_result)
                }
                STREAM_PROTOCOL_TOOL => {
                    let request = EmbeddedStreamProtocolRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let input = request.into_input().map_err(ToolError::invalid_request)?;
                    let stream = self.made.stream_protocol(input).await?;
                    collect_protocol_progress(stream)
                        .await
                        .map(tool_success_result)
                }
                PULL_PROTOCOL_EVENTS_TOOL => {
                    let request = EmbeddedPullProtocolEventsRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let output = self
                        .made
                        .pull_events(request.consumer, request.limit, request.acknowledge_through)
                        .await?;
                    present_pulled_protocol_events(&output).map(tool_success_result)
                }
                VERIFY_PROTOCOL_JOURNAL_TOOL => {
                    let request = EmbeddedProtocolIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let verdict = self
                        .made
                        .verify_journal(&request.into_protocol_id())
                        .await?;
                    Ok(tool_success_result(present_protocol_journal_verdict(
                        &verdict,
                    )))
                }
                GET_PROTOCOL_TRANSCRIPT_TOOL => {
                    let request = EmbeddedProtocolIdRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let transcript = self.made.transcript(&request.into_protocol_id()).await?;
                    Ok(tool_success_result(present_protocol_transcript(
                        &transcript,
                    )))
                }
                GENERATE_PROTOCOL_REPORT_TOOL => {
                    // A mapper, not a composer: the projection is the
                    // use case's (ADR-006), and the server renders the
                    // same one.
                    let input = EmbeddedGenerateProtocolReportRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?
                        .into_input();
                    let report = self.made.report(input).await?;
                    Ok(tool_success_result(present_protocol_report(&report)))
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
                LIST_PROTOCOL_INSTANCES_TOOL => self.present_instances().await,
                SEARCH_PROTOCOL_INSTANCES_TOOL => {
                    let request_id =
                        AuthorizationRequestId::new(uuid::Uuid::new_v4().simple().to_string())
                            .map_err(|error| ToolError::invalid_request(error.to_string()))?;
                    self.search_and_present(arguments, request_id).await
                }
                REQUEST_PROTOCOL_INTERVENTION_TOOL => {
                    let request = EmbeddedRequestProtocolInterventionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                RESPOND_TO_PROTOCOL_INTERVENTION_TOOL => {
                    let request = EmbeddedRespondToProtocolInterventionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                CLOSE_PROTOCOL_INTERVENTION_TOOL => {
                    let request = EmbeddedCloseProtocolInterventionRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
                }
                COLLECT_PROTOCOL_EVIDENCE_TOOL => {
                    let request = EmbeddedCollectProtocolEvidenceRequest::try_from(arguments)
                        .map_err(ToolError::invalid_request)?;
                    let protocol_id = request.execute(&self.made).await?;
                    self.present_instance(&protocol_id).await
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
        embedded_authorized_dispatch::call(self, name, arguments, trace)
    }
}
