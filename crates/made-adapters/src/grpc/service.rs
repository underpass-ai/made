//! gRPC service handler — thin translation from proto RPCs onto
//! use cases in [`made_app`].

use std::sync::Arc;

use async_trait::async_trait;
use made_app::services::AutoDispatchService;
use made_app::usecases::{
    ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase, AssertCeremonyReasonUseCase,
    BindCeremonyParticipantsUseCase, CeremonyDraftView, CeremonyInstanceView,
    CloseCeremonyInterventionUseCase, CollectCeremonyEvidenceUseCase, CreateCouncilInput,
    CreateCouncilUseCase, DeferCeremonyGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffCeremonyDefinitionsUseCase, GetCeremonyInstanceUseCase, GetDeliberationUseCase,
    ListCeremonyInstancesUseCase, ListCouncilsUseCase, OrchestrateUseCase,
    PrepareCeremonyParticipantsUseCase, PublishCeremonyDefinitionUseCase, RegisterAgentUseCase,
    RequestCeremonyInterventionUseCase, ResolveCeremonyDefinitionUseCase,
    RespondToCeremonyInterventionUseCase, RunCeremonyStepUseCase, RunCeremonyUseCase,
    RunCouncilDecisionUseCase, StartCeremonyUseCase, StartPublishedCeremonyUseCase,
    UnregisterAgentUseCase,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionRepositoryPort, ContractRegistryPort, StatisticsPort};
use made_core::value_objects::{AgentId, CeremonyId, OutputContractId, Specialty, TaskId};
use made_proto::v1 as pb;
use made_proto::v1::made_service_server::{MadeService, MadeServiceServer};
use tonic::{Request, Response, Status};
use tracing::debug;

use super::mappers::{
    apply_ceremony_transition_input_from_proto, approve_ceremony_guard_input_from_proto,
    assert_ceremony_reason_input_from_proto, bind_ceremony_participants_input_from_proto,
    ceremony_definition_source_from_proto, ceremony_instance_state_from,
    close_ceremony_intervention_input_from_proto, collect_ceremony_evidence_input_from_proto,
    council_summary_from, defer_ceremony_guard_input_from_proto, deliberate_response_from,
    diff_ceremony_definitions_response_from, explain_ceremony_draft_response_from,
    orchestrate_response_from, output_contract_from_proto, output_contract_to_proto,
    publish_ceremony_definition_response_from, request_ceremony_intervention_input_from_proto,
    respond_to_ceremony_intervention_input_from_proto, run_ceremony_input_from_proto,
    run_ceremony_response_from, run_ceremony_step_input_from_proto,
    run_council_decision_input_from_proto, run_council_decision_response_from,
    start_ceremony_from_proto, start_published_ceremony_input_from_proto, task_from_proto,
    trigger_event_from_proto, validate_ceremony_draft_response_from, StartCeremonyFromYaml,
};
use super::status::domain_error_to_status;
use super::tracecontext::link_span_to_metadata;
use super::MadeGrpcServiceBuilder;
use crate::ceremony::CeremonyParticipantPlanAdapter;
use crate::yaml::CeremonyDefinitionYaml;

use descriptor_error::DescriptorError;
use register_agent_descriptor::descriptor_from_register_request;
use statistics_mapper::statistics_to_proto;

mod authoring_handlers;
mod ceremony_handlers;
mod council_handlers;
mod descriptor_error;
mod register_agent_descriptor;
mod statistics_mapper;

/// The gRPC service struct. Clone-friendly: every dependency is an
/// `Arc` so multiple request tasks can share state without locking.
#[derive(Clone)]
pub struct MadeGrpcService {
    pub(super) deliberate: Arc<DeliberateUseCase>,
    pub(super) orchestrate: Arc<OrchestrateUseCase>,
    pub(super) create_council: Arc<CreateCouncilUseCase>,
    pub(super) delete_council: Arc<DeleteCouncilUseCase>,
    pub(super) list_councils: Arc<ListCouncilsUseCase>,
    pub(super) get_deliberation: Arc<GetDeliberationUseCase>,
    pub(super) register_agent: Arc<RegisterAgentUseCase>,
    pub(super) unregister_agent: Arc<UnregisterAgentUseCase>,
    pub(super) run_council_decision: Arc<RunCouncilDecisionUseCase>,
    pub(super) run_ceremony: Arc<RunCeremonyUseCase>,
    pub(super) get_ceremony_instance: Arc<GetCeremonyInstanceUseCase>,
    pub(super) list_ceremony_instances: Arc<ListCeremonyInstancesUseCase>,
    pub(super) resolve_ceremony_definition: Arc<ResolveCeremonyDefinitionUseCase>,
    pub(super) start_ceremony: Arc<StartCeremonyUseCase>,
    pub(super) start_published_ceremony: Arc<StartPublishedCeremonyUseCase>,
    pub(super) run_ceremony_step: Arc<RunCeremonyStepUseCase>,
    pub(super) apply_ceremony_transition: Arc<ApplyCeremonyTransitionUseCase>,
    pub(super) approve_ceremony_guard: Arc<ApproveCeremonyGuardUseCase>,
    pub(super) defer_ceremony_guard: Arc<DeferCeremonyGuardUseCase>,
    pub(super) assert_ceremony_reason: Arc<AssertCeremonyReasonUseCase>,
    pub(super) request_ceremony_intervention: Arc<RequestCeremonyInterventionUseCase>,
    pub(super) respond_to_ceremony_intervention: Arc<RespondToCeremonyInterventionUseCase>,
    pub(super) close_ceremony_intervention: Arc<CloseCeremonyInterventionUseCase>,
    pub(super) collect_ceremony_evidence: Arc<CollectCeremonyEvidenceUseCase>,
    pub(super) diff_ceremony_definitions: Arc<DiffCeremonyDefinitionsUseCase>,
    pub(super) bind_ceremony_participants: Arc<BindCeremonyParticipantsUseCase>,
    pub(super) publish_ceremony_definition: Arc<PublishCeremonyDefinitionUseCase>,
    pub(super) ceremony_definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    pub(super) prepare_ceremony_participants: Arc<PrepareCeremonyParticipantsUseCase>,
    pub(super) contract_registry: Arc<dyn ContractRegistryPort>,
    pub(super) auto_dispatch: Arc<AutoDispatchService>,
    pub(super) statistics: Arc<dyn StatisticsPort>,
    pub(super) started_at: std::time::Instant,
    pub(super) service_version: &'static str,
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

    /// Resolve an instance's definition and derive the view every
    /// transport renders.
    ///
    /// The definition is resolved through the shared use case, so a
    /// bound instance is checked against the digest it recorded here
    /// exactly as it is in the embedded distribution.
    async fn project(
        &self,
        instance: &made_core::entities::CeremonyInstance,
    ) -> Result<pb::CeremonyInstanceState, Status> {
        let definition = self
            .resolve_ceremony_definition
            .execute(instance)
            .await
            .map_err(domain_error_to_status)?;
        Self::render(instance, &definition).map_err(domain_error_to_status)
    }

    /// Rendering a session whose definition is already in hand. A move
    /// changes the instance and never the definition, so the mutating
    /// RPCs resolve once and render with what they resolved.
    fn render(
        instance: &made_core::entities::CeremonyInstance,
        definition: &made_core::entities::CeremonyDefinition,
    ) -> Result<pb::CeremonyInstanceState, DomainError> {
        let view = CeremonyInstanceView::project(instance, definition)?;
        Ok(ceremony_instance_state_from(&view))
    }

    /// Give the session the participants its steps will deliberate
    /// with. RunCeremony does this before it runs; a session advanced
    /// one call at a time needs it just as much, and needs it once, at
    /// the start — otherwise a ceremony can be opened and then never
    /// moved, which is the worst of the two failures.
    async fn prepare_participants(
        &self,
        definition: &made_core::entities::CeremonyDefinition,
    ) -> Result<(), Status> {
        let plan = CeremonyParticipantPlanAdapter::from_definition(definition)
            .map_err(domain_error_to_status)?;
        self.prepare_ceremony_participants
            .execute(plan)
            .await
            .map_err(domain_error_to_status)?;
        Ok(())
    }

    /// Load a session together with the definition it runs — the first
    /// thing every move needs and the only place the two are paired.
    async fn session(
        &self,
        ceremony_id: &CeremonyId,
    ) -> Result<
        (
            made_core::entities::CeremonyInstance,
            made_core::entities::CeremonyDefinition,
        ),
        Status,
    > {
        let instance = self
            .get_ceremony_instance
            .execute(ceremony_id)
            .await
            .map_err(domain_error_to_status)?;
        let definition = self
            .resolve_ceremony_definition
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

#[async_trait]
impl MadeService for MadeGrpcService {
    type StreamDeliberationStream = tokio_stream::wrappers::ReceiverStream<
        std::result::Result<pb::StreamDeliberationResponse, Status>,
    >;

    async fn deliberate(
        &self,
        request: Request<pb::DeliberateRequest>,
    ) -> GrpcResult<pb::DeliberateResponse> {
        self.handle_deliberate(request).await
    }

    async fn stream_deliberation(
        &self,
        request: Request<pb::StreamDeliberationRequest>,
    ) -> GrpcResult<Self::StreamDeliberationStream> {
        self.handle_stream_deliberation(request).await
    }

    async fn get_deliberation_result(
        &self,
        request: Request<pb::GetDeliberationResultRequest>,
    ) -> GrpcResult<pb::GetDeliberationResultResponse> {
        self.handle_get_deliberation_result(request).await
    }

    async fn orchestrate(
        &self,
        request: Request<pb::OrchestrateRequest>,
    ) -> GrpcResult<pb::OrchestrateResponse> {
        self.handle_orchestrate(request).await
    }

    async fn create_council(
        &self,
        request: Request<pb::CreateCouncilRequest>,
    ) -> GrpcResult<pb::CreateCouncilResponse> {
        self.handle_create_council(request).await
    }

    async fn list_councils(
        &self,
        request: Request<pb::ListCouncilsRequest>,
    ) -> GrpcResult<pb::ListCouncilsResponse> {
        self.handle_list_councils(request).await
    }

    async fn delete_council(
        &self,
        request: Request<pb::DeleteCouncilRequest>,
    ) -> GrpcResult<pb::DeleteCouncilResponse> {
        self.handle_delete_council(request).await
    }

    async fn register_agent(
        &self,
        request: Request<pb::RegisterAgentRequest>,
    ) -> GrpcResult<pb::RegisterAgentResponse> {
        self.handle_register_agent(request).await
    }

    async fn unregister_agent(
        &self,
        request: Request<pb::UnregisterAgentRequest>,
    ) -> GrpcResult<pb::UnregisterAgentResponse> {
        self.handle_unregister_agent(request).await
    }

    async fn run_council_decision(
        &self,
        request: Request<pb::RunCouncilDecisionRequest>,
    ) -> GrpcResult<pb::RunCouncilDecisionResponse> {
        self.handle_run_council_decision(request).await
    }

    async fn register_contract(
        &self,
        request: Request<pb::RegisterContractRequest>,
    ) -> GrpcResult<pb::RegisterContractResponse> {
        self.handle_register_contract(request).await
    }

    async fn list_contracts(
        &self,
        request: Request<pb::ListContractsRequest>,
    ) -> GrpcResult<pb::ListContractsResponse> {
        self.handle_list_contracts(request).await
    }

    async fn delete_contract(
        &self,
        request: Request<pb::DeleteContractRequest>,
    ) -> GrpcResult<pb::DeleteContractResponse> {
        self.handle_delete_contract(request).await
    }

    async fn process_trigger_event(
        &self,
        request: Request<pb::ProcessTriggerEventRequest>,
    ) -> GrpcResult<pb::ProcessTriggerEventResponse> {
        self.handle_process_trigger_event(request).await
    }

    async fn run_ceremony(
        &self,
        request: Request<pb::RunCeremonyRequest>,
    ) -> GrpcResult<pb::RunCeremonyResponse> {
        self.handle_run_ceremony(request).await
    }

    async fn start_ceremony(
        &self,
        request: Request<pb::StartCeremonyRequest>,
    ) -> GrpcResult<pb::StartCeremonyResponse> {
        self.handle_start_ceremony(request).await
    }

    async fn start_published_ceremony(
        &self,
        request: Request<pb::StartPublishedCeremonyRequest>,
    ) -> GrpcResult<pb::StartPublishedCeremonyResponse> {
        self.handle_start_published_ceremony(request).await
    }

    async fn run_ceremony_step(
        &self,
        request: Request<pb::RunCeremonyStepRequest>,
    ) -> GrpcResult<pb::RunCeremonyStepResponse> {
        self.handle_run_ceremony_step(request).await
    }

    async fn apply_ceremony_transition(
        &self,
        request: Request<pb::ApplyCeremonyTransitionRequest>,
    ) -> GrpcResult<pb::ApplyCeremonyTransitionResponse> {
        self.handle_apply_ceremony_transition(request).await
    }

    async fn approve_ceremony_guard(
        &self,
        request: Request<pb::ApproveCeremonyGuardRequest>,
    ) -> GrpcResult<pb::ApproveCeremonyGuardResponse> {
        self.handle_approve_ceremony_guard(request).await
    }

    async fn defer_ceremony_guard(
        &self,
        request: Request<pb::DeferCeremonyGuardRequest>,
    ) -> GrpcResult<pb::DeferCeremonyGuardResponse> {
        self.handle_defer_ceremony_guard(request).await
    }

    async fn assert_ceremony_reason(
        &self,
        request: Request<pb::AssertCeremonyReasonRequest>,
    ) -> GrpcResult<pb::AssertCeremonyReasonResponse> {
        self.handle_assert_ceremony_reason(request).await
    }

    async fn request_ceremony_intervention(
        &self,
        request: Request<pb::RequestCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::RequestCeremonyInterventionResponse> {
        self.handle_request_ceremony_intervention(request).await
    }

    async fn respond_to_ceremony_intervention(
        &self,
        request: Request<pb::RespondToCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::RespondToCeremonyInterventionResponse> {
        self.handle_respond_to_ceremony_intervention(request).await
    }

    async fn close_ceremony_intervention(
        &self,
        request: Request<pb::CloseCeremonyInterventionRequest>,
    ) -> GrpcResult<pb::CloseCeremonyInterventionResponse> {
        self.handle_close_ceremony_intervention(request).await
    }

    async fn collect_ceremony_evidence(
        &self,
        request: Request<pb::CollectCeremonyEvidenceRequest>,
    ) -> GrpcResult<pb::CollectCeremonyEvidenceResponse> {
        self.handle_collect_ceremony_evidence(request).await
    }

    async fn validate_ceremony_draft(
        &self,
        request: Request<pb::ValidateCeremonyDraftRequest>,
    ) -> GrpcResult<pb::ValidateCeremonyDraftResponse> {
        self.handle_validate_ceremony_draft(request).await
    }

    async fn explain_ceremony_draft(
        &self,
        request: Request<pb::ExplainCeremonyDraftRequest>,
    ) -> GrpcResult<pb::ExplainCeremonyDraftResponse> {
        self.handle_explain_ceremony_draft(request).await
    }

    async fn publish_ceremony_definition(
        &self,
        request: Request<pb::PublishCeremonyDefinitionRequest>,
    ) -> GrpcResult<pb::PublishCeremonyDefinitionResponse> {
        self.handle_publish_ceremony_definition(request).await
    }

    async fn bind_ceremony_participants(
        &self,
        request: Request<pb::BindCeremonyParticipantsRequest>,
    ) -> GrpcResult<pb::BindCeremonyParticipantsResponse> {
        self.handle_bind_ceremony_participants(request).await
    }

    async fn diff_ceremony_definitions(
        &self,
        request: Request<pb::DiffCeremonyDefinitionsRequest>,
    ) -> GrpcResult<pb::DiffCeremonyDefinitionsResponse> {
        self.handle_diff_ceremony_definitions(request).await
    }

    async fn get_ceremony_instance(
        &self,
        request: Request<pb::GetCeremonyInstanceRequest>,
    ) -> GrpcResult<pb::GetCeremonyInstanceResponse> {
        self.handle_get_ceremony_instance(request).await
    }

    async fn list_ceremony_instances(
        &self,
        request: Request<pb::ListCeremonyInstancesRequest>,
    ) -> GrpcResult<pb::ListCeremonyInstancesResponse> {
        self.handle_list_ceremony_instances(request).await
    }

    async fn get_status(
        &self,
        request: Request<pb::GetStatusRequest>,
    ) -> GrpcResult<pb::GetStatusResponse> {
        self.handle_get_status(request).await
    }

    async fn get_metrics(
        &self,
        request: Request<pb::GetMetricsRequest>,
    ) -> GrpcResult<pb::GetMetricsResponse> {
        self.handle_get_metrics(request).await
    }
}

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
