//! gRPC service handler — thin translation from proto RPCs onto
//! use cases in [`made_app`].

use std::sync::Arc;

use made_app::artifacts::ArtifactService;
use made_app::authorization::{
    AcceptedStepCompletion, AuthorizationPolicyAdministrationService,
    ContinueAcceptedStepClaimUseCase, ReadAuthorizationDecisionsUseCase,
    ReadAuthorizationPolicyUseCase,
};
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedProtocolUseCase,
};
use made_app::services::AutoDispatchService;
use made_app::usecases::SearchProtocolInstancesUseCase;
use made_app::usecases::{
    AcceptChildCompletionUseCase, AcknowledgeProtocolAgentInterventionUseCase,
    ApplyProtocolTransitionUseCase, ApproveProtocolGuardUseCase, AssertProtocolReasonUseCase,
    BindProtocolParticipantsUseCase, CancelProtocolUseCase, CloseProtocolInterventionUseCase,
    CollectProtocolEvidenceUseCase, CompleteProtocolStepUseCase, CreateCouncilInput,
    CreateCouncilUseCase, DeferProtocolGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffProtocolDefinitionsUseCase, EnforceProtocolDeadlinesUseCase, GenerateProtocolReportUseCase,
    GetDeliberationUseCase, GetProtocolInstanceUseCase, GetProtocolInterventionUseCase,
    GetProtocolTranscriptUseCase, GetServiceMetricsUseCase, GetServiceStatusUseCase,
    ListCouncilsUseCase, ListProtocolInstancesUseCase, ListProtocolInterventionsUseCase,
    OrchestrateUseCase, PauseProtocolUseCase, PrepareProtocolParticipantsUseCase,
    ProtocolAgentStatusService, ProtocolDraftView, ProtocolInstanceView,
    PublishProtocolDefinitionUseCase, PullProtocolAgentInterventionsUseCase,
    PullProtocolEventsUseCase, ReadProtocolEventsUseCase, RecoverProtocolChildrenUseCase,
    RegisterAgentUseCase, RequestProtocolInterventionUseCase, ResolveProtocolDefinitionUseCase,
    RespondToProtocolInterventionUseCase, ResumeProtocolUseCase, RunCouncilDecisionUseCase,
    RunProtocolStepUseCase, RunProtocolUseCase, StartProtocolStepUseCase, StartProtocolUseCase,
    StartPublishedProtocolUseCase, StreamProtocolUseCase, UnregisterAgentUseCase,
    VerifyProtocolJournalUseCase,
};
use made_app::workers::{
    CompleteExecutionReceiptUseCase, GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::error::DomainError;
use made_core::ports::{
    ArtifactUploadId, ClockPort, ContractRegistryPort, ProtocolDefinitionRepositoryPort,
};
use made_core::value_objects::{
    AgentId, ArtifactId, AuthorizationAction, AuthorizationScope, AuthorizedOperation, CouncilId,
    MaxParallel, OutputContractId, ProtocolId, Specialty, TaskId,
};
use made_proto::v1 as pb;
use made_proto::v1::made_service_server::{MadeService, MadeServiceServer};
use tonic::{Request, Response, Status};
use tracing::debug;

use super::mappers::{
    adopt_execution_receipt_input_from_proto, apply_protocol_transition_input_from_proto,
    approve_protocol_guard_input_from_proto, artifact_chunk_to_proto,
    artifact_page_limit_from_proto, artifact_record_to_proto, artifact_ref_to_proto,
    artifact_tombstone_to_proto, artifact_upload_status_to_proto,
    assert_protocol_reason_input_from_proto, begin_artifact_upload_from_proto,
    bind_protocol_participants_input_from_proto, budget_balance_to_proto, budget_limits_from_proto,
    budget_reservation_estimate_from_proto, budget_reservation_to_proto,
    cancel_protocol_input_from_proto, child_completion_state_from,
    claim_protocol_step_input_from_proto, close_protocol_intervention_input_from_proto,
    collect_protocol_evidence_input_from_proto, complete_execution_receipt_input_from_proto,
    complete_protocol_step_input_from_proto, council_summary_from,
    defer_protocol_guard_input_from_proto, deliberate_response_from, design_protocol_response_from,
    diff_protocol_definitions_response_from, enforce_protocol_deadlines_input_from_proto,
    execution_receipt_to_proto, execution_recovery_cursor_from_proto,
    execution_recovery_limit_from_proto, execution_recovery_page_to_proto,
    explain_protocol_draft_response_from, generate_protocol_report_response_from,
    get_protocol_transcript_response_from, orchestrate_response_from, output_contract_from_proto,
    output_contract_to_proto, pause_protocol_input_from_proto, protocol_agent_status_from_proto,
    protocol_agent_status_page_to_proto, protocol_agent_status_query_from_proto,
    protocol_agent_status_to_proto, protocol_definition_source_from_proto,
    protocol_design_document_from_proto, protocol_instance_state_from,
    publish_protocol_definition_response_from, pull_protocol_events_response_from,
    put_artifact_chunk_from_proto, read_artifact_chunk_from_proto,
    read_protocol_events_response_from, request_protocol_intervention_input_from_proto,
    respond_to_protocol_intervention_input_from_proto, resume_protocol_input_from_proto,
    run_council_decision_input_from_proto, run_council_decision_response_from,
    run_protocol_input_from_proto, run_protocol_response_from, run_protocol_step_input_from_proto,
    start_protocol_from_proto, start_published_protocol_input_from_proto,
    stream_protocol_response_from, task_from_proto, tombstone_artifact_from_proto,
    trigger_event_from_proto, unrehydratable_protocol_instance_state_from,
    validate_protocol_draft_response_from, verify_protocol_journal_response_from,
    StartProtocolFromYaml,
};
use super::status::{artifact_error_to_status, budget_error_to_status, domain_error_to_status};
use super::tracecontext::{
    link_span_to_metadata, run_with_protocol_trace, trace_context_from_metadata,
};
use super::{GrpcAuthorizationGate, MadeGrpcServiceBuilder};
use crate::protocol::ProtocolParticipantPlanAdapter;
use crate::yaml::ProtocolDefinitionYaml;

use descriptor_error::DescriptorError;
use metrics_snapshot_mapper::metric_family_to_proto;
use register_agent_descriptor::descriptor_from_register_request;
use statistics_mapper::{service_status_to_proto, statistics_to_proto};

mod agentic_system_handlers;
pub mod agentic_system_operations;
mod agentic_system_requests;
mod artifact_handlers;
mod authoring_handlers;
mod authorization_handlers;
mod budget_handlers;
mod council_handlers;
mod council_journal_handlers;
mod descriptor_error;
mod execution_receipt_handlers;
mod host_handoff_handlers;
mod integrator_loop_handlers;
pub mod integrator_loop_operations;
mod intervention_delivery_handlers;
mod metrics_snapshot_mapper;
mod protocol_agent_status_handlers;
mod protocol_delegation_handlers;
mod protocol_handlers;
mod protocol_history_handlers;
mod protocol_lifecycle_handlers;
mod protocol_search_handlers;
mod register_agent_descriptor;
mod rpc;
mod statistics_mapper;
mod succession_handlers;

use agentic_system_operations::AgenticSystemOperations;

/// The gRPC service struct. Clone-friendly: every dependency is an
/// `Arc` so multiple request tasks can share state without locking.
#[derive(Clone)]
pub struct MadeGrpcService {
    /// Absent when no agentic-system stores were composed; the
    /// operations then refuse honestly rather than answering with an
    /// empty catalogue.
    pub(super) agentic_system: Option<Arc<AgenticSystemOperations>>,
    pub(super) record_protocol_host_handoff:
        Option<Arc<made_app::workers::RecordProtocolHostHandoffUseCase>>,
    pub(super) inspect_protocol_resume:
        Option<Arc<made_app::workers::InspectProtocolResumeUseCase>>,
    pub(super) plan_protocol_successor:
        Option<Arc<made_app::usecases::PlanProtocolSuccessorUseCase>>,
    pub(super) start_protocol_successor:
        Option<Arc<made_app::usecases::StartProtocolSuccessorUseCase>>,
    pub(super) authorization: Arc<GrpcAuthorizationGate>,
    pub(super) authorization_administration: Arc<AuthorizationPolicyAdministrationService>,
    pub(super) read_authorization_policy: Arc<ReadAuthorizationPolicyUseCase>,
    pub(super) read_authorization_decisions: Arc<ReadAuthorizationDecisionsUseCase>,
    pub(super) continue_accepted_step_claim: Arc<ContinueAcceptedStepClaimUseCase>,
    pub(super) council_journal: Arc<made_app::services::CouncilJournalService>,
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) max_parallel_ceiling: MaxParallel,
    pub(super) deliberate: Arc<DeliberateUseCase>,
    pub(super) orchestrate: Arc<OrchestrateUseCase>,
    pub(super) create_council: Arc<CreateCouncilUseCase>,
    pub(super) delete_council: Arc<DeleteCouncilUseCase>,
    pub(super) list_councils: Arc<ListCouncilsUseCase>,
    pub(super) get_deliberation: Arc<GetDeliberationUseCase>,
    pub(super) register_agent: Arc<RegisterAgentUseCase>,
    pub(super) unregister_agent: Arc<UnregisterAgentUseCase>,
    pub(super) run_council_decision: Arc<RunCouncilDecisionUseCase>,
    pub(super) run_protocol: Arc<RunProtocolUseCase>,
    pub(super) get_protocol_instance: Arc<GetProtocolInstanceUseCase>,
    pub(super) list_protocol_instances: Arc<ListProtocolInstancesUseCase>,
    pub(super) search_protocol_instances: Arc<SearchProtocolInstancesUseCase>,
    pub(super) resolve_protocol_definition: Arc<ResolveProtocolDefinitionUseCase>,
    pub(super) start_protocol: Arc<StartProtocolUseCase>,
    pub(super) start_published_protocol: Arc<StartPublishedProtocolUseCase>,
    pub(super) start_budgeted_protocol: Option<Arc<StartBudgetedProtocolUseCase>>,
    pub(super) run_protocol_step: Arc<RunProtocolStepUseCase>,
    pub(super) accept_child_completion: Arc<AcceptChildCompletionUseCase>,
    pub(super) recover_protocol_children: Arc<RecoverProtocolChildrenUseCase>,
    pub(super) claim_protocol_step: Arc<StartProtocolStepUseCase>,
    pub(super) budgeted_step_claim: Option<Arc<BudgetedStepClaimUseCase>>,
    pub(super) complete_protocol_step: Arc<CompleteProtocolStepUseCase>,
    pub(super) renew_protocol_step_lease:
        Option<Arc<made_app::workers::RenewProtocolStepLeaseUseCase>>,
    pub(super) get_execution_receipt: Option<Arc<GetExecutionReceiptUseCase>>,
    pub(super) inspect_execution_recovery: Option<Arc<InspectExecutionRecoveryUseCase>>,
    pub(super) complete_execution_receipt: Option<Arc<CompleteExecutionReceiptUseCase>>,
    pub(super) apply_protocol_transition: Arc<ApplyProtocolTransitionUseCase>,
    pub(super) pause_protocol: Arc<PauseProtocolUseCase>,
    pub(super) resume_protocol: Arc<ResumeProtocolUseCase>,
    pub(super) cancel_protocol: Arc<CancelProtocolUseCase>,
    pub(super) enforce_protocol_deadlines: Arc<EnforceProtocolDeadlinesUseCase>,
    pub(super) approve_protocol_guard: Arc<ApproveProtocolGuardUseCase>,
    pub(super) defer_protocol_guard: Arc<DeferProtocolGuardUseCase>,
    pub(super) assert_protocol_reason: Arc<AssertProtocolReasonUseCase>,
    pub(super) request_protocol_intervention: Arc<RequestProtocolInterventionUseCase>,
    pub(super) respond_to_protocol_intervention: Arc<RespondToProtocolInterventionUseCase>,
    pub(super) close_protocol_intervention: Arc<CloseProtocolInterventionUseCase>,
    pub(super) pull_protocol_agent_interventions: Arc<PullProtocolAgentInterventionsUseCase>,
    pub(super) acknowledge_protocol_agent_intervention:
        Arc<AcknowledgeProtocolAgentInterventionUseCase>,
    pub(super) get_protocol_intervention: Arc<GetProtocolInterventionUseCase>,
    pub(super) list_protocol_interventions: Arc<ListProtocolInterventionsUseCase>,
    /// Absent when no integrator loop was composed; the five RPCs then
    /// refuse honestly rather than answering an empty queue.
    pub(super) integrator_loop: Option<Arc<integrator_loop_operations::IntegratorLoopOperations>>,
    pub(super) collect_protocol_evidence: Arc<CollectProtocolEvidenceUseCase>,
    pub(super) protocol_agent_status: Arc<ProtocolAgentStatusService>,
    pub(super) read_protocol_events: Arc<ReadProtocolEventsUseCase>,
    pub(super) stream_protocol: Arc<StreamProtocolUseCase>,
    pub(super) pull_protocol_events: Arc<PullProtocolEventsUseCase>,
    pub(super) verify_protocol_journal: Arc<VerifyProtocolJournalUseCase>,
    pub(super) get_protocol_transcript: Arc<GetProtocolTranscriptUseCase>,
    pub(super) generate_protocol_report: Arc<GenerateProtocolReportUseCase>,
    pub(super) diff_protocol_definitions: Arc<DiffProtocolDefinitionsUseCase>,
    pub(super) protocol_publications: Arc<dyn made_core::ports::ProtocolDefinitionPublicationPort>,
    pub(super) bind_protocol_participants: Arc<BindProtocolParticipantsUseCase>,
    pub(super) publish_protocol_definition: Arc<PublishProtocolDefinitionUseCase>,
    pub(super) protocol_definitions: Arc<dyn ProtocolDefinitionRepositoryPort>,
    pub(super) prepare_protocol_participants: Arc<PrepareProtocolParticipantsUseCase>,
    pub(super) contract_registry: Arc<dyn ContractRegistryPort>,
    pub(super) auto_dispatch: Arc<AutoDispatchService>,
    /// Observability is two use cases like everything else here.
    /// The version, the uptime and the counter port they read are the
    /// builder's to compose, so this service holds no clock and no
    /// version of its own: an answer assembled in a handler is an
    /// answer the in-process edition cannot give (ADR-014).
    pub(super) get_service_status: Arc<GetServiceStatusUseCase>,
    pub(super) get_service_metrics: Arc<GetServiceMetricsUseCase>,
    pub(super) artifacts: Option<Arc<ArtifactService>>,
    pub(super) budgets: Option<Arc<BudgetLedgerService>>,
}

impl std::fmt::Debug for MadeGrpcService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MadeGrpcService").finish()
    }
}

impl MadeGrpcService {
    #[must_use]
    pub fn builder() -> MadeGrpcServiceBuilder {
        MadeGrpcServiceBuilder::default()
    }

    /// Authorization boundary shared by every typed RPC handler.
    #[must_use]
    pub const fn authorization(&self) -> &Arc<GrpcAuthorizationGate> {
        &self.authorization
    }

    async fn authorize_global<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
    ) -> Result<AuthorizedOperation, Status> {
        self.authorization
            .authorize(request, action, AuthorizationScope::Global, None)
            .await
    }

    async fn authorize_definition<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        definition_yaml: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let draft = ProtocolDefinitionYaml::parse_draft_str(definition_yaml)
            .map_err(domain_error_to_status)?;
        self.authorization
            .authorize(
                request,
                action,
                AuthorizationScope::Definition {
                    name: draft.name().clone(),
                    version: Some(draft.version().clone()),
                },
                None,
            )
            .await
    }

    async fn authorize_protocol<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        protocol_id: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let protocol_id = ProtocolId::new(protocol_id).map_err(domain_error_to_status)?;
        let scope = match self.get_protocol_instance.execute(&protocol_id).await {
            Ok(instance) => instance.lineage().map_or_else(
                || AuthorizationScope::Protocol {
                    protocol_id: protocol_id.clone(),
                },
                |lineage| AuthorizationScope::ResolvedProtocol {
                    protocol_id: protocol_id.clone(),
                    root_id: lineage.root_id().clone(),
                },
            ),
            Err(DomainError::NotFound { .. }) => AuthorizationScope::Protocol {
                protocol_id: protocol_id.clone(),
            },
            Err(error) => return Err(domain_error_to_status(error)),
        };
        self.authorization
            .authorize_authenticated(request, principal, action, scope, None)
            .await
    }

    async fn authorize_child_parent<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        child_id: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let child_id = ProtocolId::new(child_id).map_err(domain_error_to_status)?;
        let child = self
            .get_protocol_instance
            .execute(&child_id)
            .await
            .map_err(domain_error_to_status)?;
        let parent_id = child
            .lineage()
            .ok_or_else(|| {
                domain_error_to_status(DomainError::InvariantViolated {
                    reason: "child authorization scope requires sealed lineage",
                })
            })?
            .parent_id()
            .clone();
        let parent = self
            .get_protocol_instance
            .execute(&parent_id)
            .await
            .map_err(domain_error_to_status)?;
        let scope = parent.lineage().map_or_else(
            || AuthorizationScope::Protocol {
                protocol_id: parent_id.clone(),
            },
            |lineage| AuthorizationScope::ResolvedProtocol {
                protocol_id: parent_id.clone(),
                root_id: lineage.root_id().clone(),
            },
        );
        self.authorization
            .authorize_authenticated(request, principal, action, scope, None)
            .await
    }

    async fn authorize_council<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        council_id: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let council_id = CouncilId::new(council_id).map_err(domain_error_to_status)?;
        self.authorization
            .authorize_authenticated(
                request,
                principal,
                action,
                AuthorizationScope::Council { council_id },
                None,
            )
            .await
    }

    async fn continue_accepted_step_completion(
        &self,
        request: &Request<pb::CompleteProtocolStepRequest>,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let request_id = self
            .authorization
            .request_id(request, AuthorizationAction::CompleteProtocolStep)?;
        let target_digest = self
            .authorization
            .target_digest_for_authenticated(request, &principal)
            .map_err(Status::from)?;
        let input = AcceptedStepCompletion::from_invocation(
            ProtocolId::new(&request.get_ref().protocol_id).map_err(domain_error_to_status)?,
            made_core::value_objects::StepId::new(&request.get_ref().step_id)
                .map_err(domain_error_to_status)?,
            made_core::value_objects::StepClaimFence::new(&request.get_ref().claim_fence)
                .map_err(domain_error_to_status)?,
            principal,
            &request_id,
            target_digest,
        )
        .map_err(domain_error_to_status)?;
        self.continue_accepted_step_claim
            .execute(input)
            .await
            .map_err(domain_error_to_status)
    }

    async fn authorize_artifact<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        artifact_id: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let artifact_id = ArtifactId::new(artifact_id).map_err(domain_error_to_status)?;
        self.authorization
            .authorize_authenticated(
                request,
                principal,
                action,
                AuthorizationScope::Artifact { artifact_id },
                None,
            )
            .await
    }

    async fn authorize_artifact_upload<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        upload_id: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let upload_id = ArtifactUploadId::new(upload_id).map_err(domain_error_to_status)?;
        let artifact_id = self
            .artifact_service()
            .ok_or_else(|| Status::unavailable("artifact storage is not configured"))?
            .artifact_id_for_upload(&upload_id)
            .await
            .map_err(artifact_error_to_status)?;
        self.authorization
            .authorize_authenticated(
                request,
                principal,
                action,
                AuthorizationScope::Artifact { artifact_id },
                None,
            )
            .await
    }

    async fn authorize_artifact_upload_begin<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        requested_artifact_id: Option<&str>,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let scope = requested_artifact_id.map_or(Ok(AuthorizationScope::Global), |id| {
            ArtifactId::new(id).map(|artifact_id| AuthorizationScope::Artifact { artifact_id })
        });
        self.authorization
            .authorize_authenticated(
                request,
                principal,
                action,
                scope.map_err(domain_error_to_status)?,
                None,
            )
            .await
    }

    async fn authorize_budget_for_protocol<T: prost::Message>(
        &self,
        request: &Request<T>,
        action: AuthorizationAction,
        protocol_id: &str,
    ) -> Result<AuthorizedOperation, Status> {
        let principal = self.authorization.authenticate(request)?;
        let protocol_id = ProtocolId::new(protocol_id).map_err(domain_error_to_status)?;
        let instance = self
            .get_protocol_instance
            .execute(&protocol_id)
            .await
            .map_err(domain_error_to_status)?;
        let account_id = instance
            .budget_account_id()
            .cloned()
            .ok_or_else(|| Status::failed_precondition("protocol has no durable budget account"))?;
        self.authorization
            .authorize_authenticated(
                request,
                principal,
                action,
                AuthorizationScope::Budget { account_id },
                None,
            )
            .await
    }

    fn artifact_service(&self) -> Option<&ArtifactService> {
        self.artifacts.as_deref()
    }

    /// Resolve an instance's definition and derive the view every
    /// transport renders.
    ///
    /// The definition is resolved through the shared use case, so a
    /// bound instance is checked against the digest it recorded here
    /// exactly as it is in the embedded distribution.
    async fn project(
        &self,
        instance: &made_core::entities::ProtocolInstance,
    ) -> Result<pb::ProtocolInstanceState, Status> {
        let definition = self
            .resolve_protocol_definition
            .execute(instance)
            .await
            .map_err(domain_error_to_status)?;
        self.render(instance, &definition).await
    }

    /// Rendering a session whose definition is already in hand. A move
    /// changes the instance and never the definition, so the mutating
    /// RPCs resolve once and render with what they resolved.
    async fn render(
        &self,
        instance: &made_core::entities::ProtocolInstance,
        definition: &made_core::entities::ProtocolDefinition,
    ) -> Result<pb::ProtocolInstanceState, Status> {
        let head = self
            .get_protocol_instance
            .execute_with_ids(instance.id())
            .await
            .map_err(domain_error_to_status)?;
        self.render_read(&head, definition)
            .map_err(domain_error_to_status)
    }

    fn render_read(
        &self,
        read: &made_app::usecases::ProtocolInstanceRead,
        definition: &made_core::entities::ProtocolDefinition,
    ) -> Result<pb::ProtocolInstanceState, made_core::error::DomainError> {
        let view = ProtocolInstanceView::project_at(
            read.instance(),
            definition,
            self.clock.now(),
            self.max_parallel_ceiling,
        )?;
        let mut state = protocol_instance_state_from(&view);
        read.trace_id()
            .map_or_else(String::new, ToString::to_string)
            .clone_into(&mut state.trace_id);
        state.correlation_id = read
            .correlation_id()
            .map_or_else(String::new, ToString::to_string);
        state.causation_id = read
            .causation_id()
            .map_or_else(String::new, ToString::to_string);
        Ok(state)
    }

    /// Give the session the participants its steps will deliberate
    /// with. RunProtocol does this before it runs; a session advanced
    /// one call at a time needs it just as much, and needs it once, at
    /// the start — otherwise a protocol can be opened and then never
    /// moved, which is the worst of the two failures.
    async fn prepare_participants(
        &self,
        definition: &made_core::entities::ProtocolDefinition,
    ) -> Result<(), Status> {
        let plan = ProtocolParticipantPlanAdapter::from_definition(definition)
            .map_err(domain_error_to_status)?;
        self.prepare_protocol_participants
            .execute(plan)
            .await
            .map_err(domain_error_to_status)?;
        Ok(())
    }

    /// Load a session together with the definition it runs — the first
    /// thing every move needs and the only place the two are paired.
    async fn session(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<
        (
            made_core::entities::ProtocolInstance,
            made_core::entities::ProtocolDefinition,
        ),
        Status,
    > {
        let instance = self
            .get_protocol_instance
            .execute(protocol_id)
            .await
            .map_err(domain_error_to_status)?;
        let definition = self
            .resolve_protocol_definition
            .execute(&instance)
            .await
            .map_err(domain_error_to_status)?;
        Ok((instance, definition))
    }

    /// Wrap this service into a Tonic `Server` middleware.
    #[must_use]
    pub fn into_server(self) -> MadeServiceServer<Self> {
        MadeServiceServer::new(self)
    }
}

type GrpcResult<T> = std::result::Result<Response<T>, Status>;

#[cfg(test)]
mod tests {
    use super::*;
    use made_core::entities::Statistics;
    use made_core::value_objects::{DurationMs, Specialty};

    #[test]
    fn statistics_to_proto_maps_every_field() {
        let mut stats = Statistics::new();
        stats.record_deliberation(
            &Specialty::new("triage").unwrap(),
            DurationMs::from_millis(100),
        );
        stats.record_deliberation(
            &Specialty::new("triage").unwrap(),
            DurationMs::from_millis(50),
        );
        stats.record_deliberation(
            &Specialty::new("reviewer").unwrap(),
            DurationMs::from_millis(200),
        );
        stats.record_orchestration(DurationMs::from_millis(400));

        let mapped = statistics_to_proto(&stats);
        assert_eq!(mapped.total_deliberations, 3);
        assert_eq!(mapped.total_orchestrations, 1);
        assert_eq!(mapped.total_duration_ms, 750);
        // (100 + 50 + 200 + 400) / 4 ops = 187.5
        assert!((mapped.average_duration_ms - 187.5).abs() < 1e-9);
        assert_eq!(mapped.per_specialty_counts.get("triage").copied(), Some(2));
        assert_eq!(
            mapped.per_specialty_counts.get("reviewer").copied(),
            Some(1)
        );
    }

    #[test]
    fn statistics_to_proto_empty_maps_zeros_and_empty_map() {
        let stats = Statistics::default();
        let mapped = statistics_to_proto(&stats);
        assert_eq!(mapped.total_deliberations, 0);
        assert_eq!(mapped.total_orchestrations, 0);
        assert_eq!(mapped.total_duration_ms, 0);
        assert!((mapped.average_duration_ms - 0.0).abs() < f64::EPSILON);
        assert!(mapped.per_specialty_counts.is_empty());
    }

    fn summary(id: &str, specialty: &str, kind: &str) -> pb::AgentSummary {
        pb::AgentSummary {
            agent_id: id.to_owned(),
            specialty: specialty.to_owned(),
            kind: kind.to_owned(),
            attributes: None,
        }
    }

    #[test]
    fn descriptor_from_request_uses_top_level_specialty_when_present() {
        let req = pb::RegisterAgentRequest {
            specialty: "reviewer".to_owned(),
            agent: Some(summary("a1", "triage", "noop")),
            agent_config: None,
        };
        let d = descriptor_from_register_request(req).unwrap();
        assert_eq!(d.id.as_str(), "a1");
        assert_eq!(d.specialty.as_str(), "reviewer");
        assert_eq!(d.kind.as_str(), "noop");
        assert!(d.attributes.is_empty());
    }

    #[test]
    fn descriptor_from_request_falls_back_to_nested_specialty_when_empty() {
        let req = pb::RegisterAgentRequest {
            specialty: "   ".to_owned(),
            agent: Some(summary("a1", "triage", "noop")),
            agent_config: None,
        };
        let d = descriptor_from_register_request(req).unwrap();
        assert_eq!(d.specialty.as_str(), "triage");
    }

    #[test]
    fn descriptor_from_request_missing_agent_is_reported() {
        let req = pb::RegisterAgentRequest {
            specialty: "triage".to_owned(),
            agent: None,
            agent_config: None,
        };
        let err = descriptor_from_register_request(req).unwrap_err();
        assert!(matches!(err, DescriptorError::MissingAgentSummary));
    }

    #[test]
    fn descriptor_from_request_domain_validation_propagates() {
        // Empty kind fails at AgentKind construction.
        let req = pb::RegisterAgentRequest {
            specialty: "triage".to_owned(),
            agent: Some(summary("a1", "triage", "")),
            agent_config: None,
        };
        let err = descriptor_from_register_request(req).unwrap_err();
        assert!(matches!(err, DescriptorError::Domain(_)));
    }
}
