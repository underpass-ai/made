//! Embedded MCP backend.

mod designed_ceremony_draft;
mod embedded_apply_ceremony_transition_request;
mod embedded_approve_ceremony_guard_request;
mod embedded_assert_ceremony_reason_request;
mod embedded_bind_ceremony_participants_request;
mod embedded_ceremony_draft_presenter;
mod embedded_ceremony_draft_request;
mod embedded_ceremony_instance_presenter;
mod embedded_ceremony_report_presenter;
mod embedded_claim_ceremony_step_request;
mod embedded_close_ceremony_intervention_request;
mod embedded_collect_ceremony_evidence_request;
mod embedded_complete_ceremony_step_request;
mod embedded_defer_ceremony_guard_request;
mod embedded_design_ceremony_request;
mod embedded_diff_ceremony_definitions_request;
mod embedded_generate_ceremony_report_request;
mod embedded_get_ceremony_instance_request;
mod embedded_publication_presenter;
mod embedded_publish_ceremony_definition_request;
mod embedded_request_ceremony_intervention_request;
mod embedded_request_fields;
mod embedded_respond_to_ceremony_intervention_request;
mod embedded_run_ceremony_presenter;
mod embedded_run_ceremony_request;
mod embedded_run_ceremony_step_request;
mod embedded_start_ceremony_request;
mod embedded_start_published_ceremony_request;

use made_app::usecases::CeremonyDraftView;
use made_core::value_objects::CeremonyId;
use made_embedded::EmbeddedMade;
use serde_json::Value;

use crate::backend::{MadeMcpToolBackend, MadeMcpToolFuture};
use crate::protocol::{
    tool_success_result, APPLY_CEREMONY_TRANSITION_TOOL, APPROVE_CEREMONY_GUARD_TOOL,
    ASSERT_CEREMONY_REASON_TOOL, BIND_CEREMONY_PARTICIPANTS_TOOL, CLAIM_CEREMONY_STEP_TOOL,
    CLOSE_CEREMONY_INTERVENTION_TOOL, COLLECT_CEREMONY_EVIDENCE_TOOL, COMPLETE_CEREMONY_STEP_TOOL,
    DEFER_CEREMONY_GUARD_TOOL, DESIGN_CEREMONY_TOOL, DIFF_CEREMONY_DEFINITIONS_TOOL,
    EXPLAIN_CEREMONY_DRAFT_TOOL, GENERATE_CEREMONY_REPORT_TOOL, GET_CEREMONY_INSTANCE_TOOL,
    LIST_CEREMONY_INSTANCES_TOOL, PUBLISH_CEREMONY_DEFINITION_TOOL,
    REQUEST_CEREMONY_INTERVENTION_TOOL, RESPOND_TO_CEREMONY_INTERVENTION_TOOL,
    RUN_CEREMONY_STEP_TOOL, RUN_CEREMONY_TOOL, START_CEREMONY_TOOL, START_PUBLISHED_CEREMONY_TOOL,
    VALIDATE_CEREMONY_DRAFT_TOOL,
};

use self::designed_ceremony_draft::DesignedCeremonyDraft;
use self::embedded_apply_ceremony_transition_request::EmbeddedApplyCeremonyTransitionRequest;
use self::embedded_approve_ceremony_guard_request::EmbeddedApproveCeremonyGuardRequest;
use self::embedded_assert_ceremony_reason_request::EmbeddedAssertCeremonyReasonRequest;
use self::embedded_bind_ceremony_participants_request::EmbeddedBindCeremonyParticipantsRequest;
use self::embedded_ceremony_draft_presenter::{
    present_definition_diff, EmbeddedCeremonyDraftPresenter,
};
use self::embedded_ceremony_draft_request::EmbeddedCeremonyDraftRequest;
use self::embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter;
use self::embedded_ceremony_report_presenter::EmbeddedCeremonyReportPresenter;
use self::embedded_claim_ceremony_step_request::EmbeddedClaimCeremonyStepRequest;
use self::embedded_close_ceremony_intervention_request::EmbeddedCloseCeremonyInterventionRequest;
use self::embedded_collect_ceremony_evidence_request::EmbeddedCollectCeremonyEvidenceRequest;
use self::embedded_complete_ceremony_step_request::EmbeddedCompleteCeremonyStepRequest;
use self::embedded_defer_ceremony_guard_request::EmbeddedDeferCeremonyGuardRequest;
use self::embedded_design_ceremony_request::EmbeddedDesignCeremonyRequest;
use self::embedded_diff_ceremony_definitions_request::EmbeddedDiffCeremonyDefinitionsRequest;
use self::embedded_generate_ceremony_report_request::EmbeddedGenerateCeremonyReportRequest;
use self::embedded_get_ceremony_instance_request::EmbeddedGetCeremonyInstanceRequest;
use self::embedded_publication_presenter::EmbeddedPublicationPresenter;
use self::embedded_publish_ceremony_definition_request::EmbeddedPublishCeremonyDefinitionRequest;
use self::embedded_request_ceremony_intervention_request::EmbeddedRequestCeremonyInterventionRequest;
use self::embedded_respond_to_ceremony_intervention_request::EmbeddedRespondToCeremonyInterventionRequest;
use self::embedded_run_ceremony_presenter::EmbeddedRunCeremonyPresenter;
use self::embedded_run_ceremony_request::EmbeddedRunCeremonyRequest;
use self::embedded_run_ceremony_step_request::EmbeddedRunCeremonyStepRequest;
use self::embedded_start_ceremony_request::EmbeddedStartCeremonyRequest;
use self::embedded_start_published_ceremony_request::EmbeddedStartPublishedCeremonyRequest;

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

    async fn present_instance(&self, ceremony_id: &CeremonyId) -> Result<Value, String> {
        EmbeddedCeremonyInstancePresenter::present(&self.made, ceremony_id)
            .await
            .map(tool_success_result)
    }

    async fn present_instances(&self) -> Result<Value, String> {
        let instances = self
            .made
            .instances()
            .await
            .map_err(|error| error.to_string())?;
        let mut values = Vec::with_capacity(instances.len());
        for instance in instances {
            // An instance whose definition is not in this store cannot be
            // rehydrated — the published-definition restart boundary. The
            // state is still there, so the listing reports that one entry
            // as unreadable instead of taking every readable ceremony down
            // with it. Asking for it by id still fails loudly.
            match EmbeddedCeremonyInstancePresenter::present(&self.made, instance.id()).await {
                Ok(value) => values.push(value),
                Err(reason) => values.push(serde_json::json!({
                    "ceremony_id": instance.id().as_str(),
                    "rehydratable": false,
                    "reason": reason,
                })),
            }
        }
        let count = values.len();
        Ok(tool_success_result(serde_json::json!({
            "count": count,
            "instances": values,
        })))
    }
}

impl MadeMcpToolBackend for EmbeddedMadeMcpBackend {
    fn backend_name(&self) -> &'static str {
        "embedded"
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
                | GENERATE_CEREMONY_REPORT_TOOL
        )
    }

    // A dispatch table: one arm per tool, and splitting it would put
    // half the surface somewhere a reader has to go looking for.
    #[allow(clippy::too_many_lines)]
    fn call_tool<'a>(&'a self, name: &'a str, arguments: &'a Value) -> MadeMcpToolFuture<'a> {
        Box::pin(async move {
            match name {
                DESIGN_CEREMONY_TOOL => {
                    let designed = EmbeddedDesignCeremonyRequest::try_from(arguments)?.design()?;
                    let report = designed.draft().analyze();
                    Ok(tool_success_result(
                        EmbeddedCeremonyDraftPresenter::present_design(
                            designed.definition_yaml(),
                            &CeremonyDraftView::project(designed.draft(), &report),
                            designed.stage_count(),
                            designed.participant_count(),
                            designed.final_approval_required(),
                        ),
                    ))
                }
                VALIDATE_CEREMONY_DRAFT_TOOL => {
                    let request = EmbeddedCeremonyDraftRequest::try_from(arguments)?;
                    let draft = request.parse()?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedCeremonyDraftPresenter::present_validation(
                            &CeremonyDraftView::project(&draft, &report),
                        ),
                    ))
                }
                EXPLAIN_CEREMONY_DRAFT_TOOL => {
                    let request = EmbeddedCeremonyDraftRequest::try_from(arguments)?;
                    let draft = request.parse()?;
                    let report = draft.analyze();
                    Ok(tool_success_result(
                        EmbeddedCeremonyDraftPresenter::present_explanation(
                            &CeremonyDraftView::project(&draft, &report),
                        ),
                    ))
                }
                BIND_CEREMONY_PARTICIPANTS_TOOL => {
                    let request = EmbeddedBindCeremonyParticipantsRequest::try_from(arguments)?;
                    let instance = request.execute(&self.made).await?;
                    self.present_instance(instance.id()).await
                }
                DIFF_CEREMONY_DEFINITIONS_TOOL => {
                    let request = EmbeddedDiffCeremonyDefinitionsRequest::try_from(arguments)?;
                    let diff = request.execute(&self.made).await?;
                    Ok(tool_success_result(present_definition_diff(&diff)))
                }
                PUBLISH_CEREMONY_DEFINITION_TOOL => {
                    let request = EmbeddedPublishCeremonyDefinitionRequest::try_from(arguments)?;
                    let outcome = request.execute(&self.made).await?;
                    Ok(tool_success_result(EmbeddedPublicationPresenter::present(
                        &outcome,
                    )))
                }
                START_PUBLISHED_CEREMONY_TOOL => {
                    let request = EmbeddedStartPublishedCeremonyRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RUN_CEREMONY_TOOL => {
                    let request = EmbeddedRunCeremonyRequest::try_from(arguments)?;
                    let output = request.execute(&self.made).await?;
                    Ok(tool_success_result(EmbeddedRunCeremonyPresenter::present(
                        &output,
                    )))
                }
                START_CEREMONY_TOOL => {
                    let request = EmbeddedStartCeremonyRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RUN_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedRunCeremonyStepRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                CLAIM_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedClaimCeremonyStepRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                COMPLETE_CEREMONY_STEP_TOOL => {
                    let request = EmbeddedCompleteCeremonyStepRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                APPROVE_CEREMONY_GUARD_TOOL => {
                    let request = EmbeddedApproveCeremonyGuardRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                ASSERT_CEREMONY_REASON_TOOL => {
                    let request = EmbeddedAssertCeremonyReasonRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                DEFER_CEREMONY_GUARD_TOOL => {
                    let request = EmbeddedDeferCeremonyGuardRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                APPLY_CEREMONY_TRANSITION_TOOL => {
                    let request = EmbeddedApplyCeremonyTransitionRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                GET_CEREMONY_INSTANCE_TOOL => {
                    let request = EmbeddedGetCeremonyInstanceRequest::try_from(arguments)?;
                    let ceremony_id = request.into_ceremony_id();
                    self.present_instance(&ceremony_id).await
                }
                GENERATE_CEREMONY_REPORT_TOOL => {
                    let request = EmbeddedGenerateCeremonyReportRequest::try_from(arguments)?;
                    EmbeddedCeremonyReportPresenter::present(&self.made, &request)
                        .await
                        .map(tool_success_result)
                }
                LIST_CEREMONY_INSTANCES_TOOL => self.present_instances().await,
                REQUEST_CEREMONY_INTERVENTION_TOOL => {
                    let request = EmbeddedRequestCeremonyInterventionRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                RESPOND_TO_CEREMONY_INTERVENTION_TOOL => {
                    let request =
                        EmbeddedRespondToCeremonyInterventionRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                CLOSE_CEREMONY_INTERVENTION_TOOL => {
                    let request = EmbeddedCloseCeremonyInterventionRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                COLLECT_CEREMONY_EVIDENCE_TOOL => {
                    let request = EmbeddedCollectCeremonyEvidenceRequest::try_from(arguments)?;
                    let ceremony_id = request.execute(&self.made).await?;
                    self.present_instance(&ceremony_id).await
                }
                _ => Err(format!("embedded backend: unsupported tool `{name}`")),
            }
        })
    }
}
