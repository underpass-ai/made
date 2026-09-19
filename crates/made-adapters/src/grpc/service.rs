//! gRPC service handler — thin translation from proto RPCs onto
//! use cases in [`made_app`].

use std::sync::Arc;

use made_app::artifacts::ArtifactService;
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedCeremonyUseCase,
};
use made_app::services::AutoDispatchService;
use made_app::usecases::{
    AcceptChildCompletionUseCase, ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase,
    AssertCeremonyReasonUseCase, BindCeremonyParticipantsUseCase, CancelCeremonyUseCase,
    CeremonyDraftView, CeremonyInstanceView, CloseCeremonyInterventionUseCase,
    CollectCeremonyEvidenceUseCase, CompleteCeremonyStepUseCase, CreateCouncilInput,
    CreateCouncilUseCase, DeferCeremonyGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffCeremonyDefinitionsUseCase, EnforceCeremonyDeadlinesUseCase, GenerateCeremonyReportUseCase,
    GetCeremonyInstanceUseCase, GetCeremonyTranscriptUseCase, GetDeliberationUseCase,
    GetServiceMetricsUseCase, GetServiceStatusUseCase, ListCeremonyInstancesUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, PauseCeremonyUseCase,
    PrepareCeremonyParticipantsUseCase, PublishCeremonyDefinitionUseCase,
    PullCeremonyEventsUseCase, ReadCeremonyEventsUseCase, RecoverCeremonyChildrenUseCase,
    RegisterAgentUseCase, RequestCeremonyInterventionUseCase, ResolveCeremonyDefinitionUseCase,
    RespondToCeremonyInterventionUseCase, ResumeCeremonyUseCase, RunCeremonyStepUseCase,
    RunCeremonyUseCase, RunCouncilDecisionUseCase, StartCeremonyStepUseCase, StartCeremonyUseCase,
    StartPublishedCeremonyUseCase, StreamCeremonyUseCase, UnregisterAgentUseCase,
    VerifyCeremonyJournalUseCase,
};
use made_app::workers::{
    CompleteExecutionReceiptUseCase, GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyDefinitionRepositoryPort, ClockPort, ContractRegistryPort};
use made_core::value_objects::{
    AgentId, CeremonyId, MaxParallel, OutputContractId, Specialty, TaskId,
};
use made_proto::v1 as pb;
use made_proto::v1::made_service_server::{MadeService, MadeServiceServer};
use tonic::{Request, Response, Status};
use tracing::debug;

use super::mappers::{
    adopt_execution_receipt_input_from_proto, apply_ceremony_transition_input_from_proto,
    approve_ceremony_guard_input_from_proto, artifact_chunk_to_proto,
    artifact_page_limit_from_proto, artifact_record_to_proto, artifact_ref_to_proto,
    artifact_tombstone_to_proto, artifact_upload_status_to_proto,
    assert_ceremony_reason_input_from_proto, begin_artifact_upload_from_proto,
    bind_ceremony_participants_input_from_proto, budget_balance_to_proto, budget_limits_from_proto,
    budget_reservation_estimate_from_proto, budget_reservation_to_proto,
    cancel_ceremony_input_from_proto, ceremony_definition_source_from_proto,
    ceremony_design_document_from_proto, ceremony_instance_state_from, child_completion_state_from,
    claim_ceremony_step_input_from_proto, close_ceremony_intervention_input_from_proto,
    collect_ceremony_evidence_input_from_proto, complete_ceremony_step_input_from_proto,
    complete_execution_receipt_input_from_proto, council_summary_from,
    defer_ceremony_guard_input_from_proto, deliberate_response_from, design_ceremony_response_from,
    diff_ceremony_definitions_response_from, enforce_ceremony_deadlines_input_from_proto,
    execution_receipt_to_proto, execution_recovery_cursor_from_proto,
    execution_recovery_limit_from_proto, execution_recovery_page_to_proto,
    explain_ceremony_draft_response_from, generate_ceremony_report_response_from,
    get_ceremony_transcript_response_from, orchestrate_response_from, output_contract_from_proto,
    output_contract_to_proto, pause_ceremony_input_from_proto,
    publish_ceremony_definition_response_from, pull_ceremony_events_response_from,
    put_artifact_chunk_from_proto, read_artifact_chunk_from_proto,
    read_ceremony_events_response_from, request_ceremony_intervention_input_from_proto,
    respond_to_ceremony_intervention_input_from_proto, resume_ceremony_input_from_proto,
    run_ceremony_input_from_proto, run_ceremony_response_from, run_ceremony_step_input_from_proto,
    run_council_decision_input_from_proto, run_council_decision_response_from,
    start_ceremony_from_proto, start_published_ceremony_input_from_proto,
    stream_ceremony_response_from, task_from_proto, tombstone_artifact_from_proto,
    trigger_event_from_proto, unrehydratable_ceremony_instance_state_from,
    validate_ceremony_draft_response_from, verify_ceremony_journal_response_from,
    StartCeremonyFromYaml,
};
use super::status::{artifact_error_to_status, budget_error_to_status, domain_error_to_status};
use super::tracecontext::{
    link_span_to_metadata, run_with_ceremony_trace, trace_context_from_metadata,
};
use super::MadeGrpcServiceBuilder;
use crate::ceremony::CeremonyParticipantPlanAdapter;
use crate::yaml::CeremonyDefinitionYaml;

use descriptor_error::DescriptorError;
use metrics_snapshot_mapper::metric_family_to_proto;
use register_agent_descriptor::descriptor_from_register_request;
use statistics_mapper::{service_status_to_proto, statistics_to_proto};

mod artifact_handlers;
mod authoring_handlers;
mod budget_handlers;
mod ceremony_delegation_handlers;
mod ceremony_handlers;
mod ceremony_history_handlers;
mod ceremony_lifecycle_handlers;
mod council_handlers;
mod council_journal_handlers;
mod descriptor_error;
mod execution_receipt_handlers;
mod metrics_snapshot_mapper;
mod register_agent_descriptor;
mod rpc;
mod statistics_mapper;

/// The gRPC service struct. Clone-friendly: every dependency is an
/// `Arc` so multiple request tasks can share state without locking.
#[derive(Clone)]
pub struct MadeGrpcService {
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
    pub(super) run_ceremony: Arc<RunCeremonyUseCase>,
    pub(super) get_ceremony_instance: Arc<GetCeremonyInstanceUseCase>,
    pub(super) list_ceremony_instances: Arc<ListCeremonyInstancesUseCase>,
    pub(super) resolve_ceremony_definition: Arc<ResolveCeremonyDefinitionUseCase>,
    pub(super) start_ceremony: Arc<StartCeremonyUseCase>,
    pub(super) start_published_ceremony: Arc<StartPublishedCeremonyUseCase>,
    pub(super) start_budgeted_ceremony: Option<Arc<StartBudgetedCeremonyUseCase>>,
    pub(super) run_ceremony_step: Arc<RunCeremonyStepUseCase>,
    pub(super) accept_child_completion: Arc<AcceptChildCompletionUseCase>,
    pub(super) recover_ceremony_children: Arc<RecoverCeremonyChildrenUseCase>,
    pub(super) claim_ceremony_step: Arc<StartCeremonyStepUseCase>,
    pub(super) budgeted_step_claim: Option<Arc<BudgetedStepClaimUseCase>>,
    pub(super) complete_ceremony_step: Arc<CompleteCeremonyStepUseCase>,
    pub(super) get_execution_receipt: Option<Arc<GetExecutionReceiptUseCase>>,
    pub(super) inspect_execution_recovery: Option<Arc<InspectExecutionRecoveryUseCase>>,
    pub(super) complete_execution_receipt: Option<Arc<CompleteExecutionReceiptUseCase>>,
    pub(super) apply_ceremony_transition: Arc<ApplyCeremonyTransitionUseCase>,
    pub(super) pause_ceremony: Arc<PauseCeremonyUseCase>,
    pub(super) resume_ceremony: Arc<ResumeCeremonyUseCase>,
    pub(super) cancel_ceremony: Arc<CancelCeremonyUseCase>,
    pub(super) enforce_ceremony_deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
    pub(super) approve_ceremony_guard: Arc<ApproveCeremonyGuardUseCase>,
    pub(super) defer_ceremony_guard: Arc<DeferCeremonyGuardUseCase>,
    pub(super) assert_ceremony_reason: Arc<AssertCeremonyReasonUseCase>,
    pub(super) request_ceremony_intervention: Arc<RequestCeremonyInterventionUseCase>,
    pub(super) respond_to_ceremony_intervention: Arc<RespondToCeremonyInterventionUseCase>,
    pub(super) close_ceremony_intervention: Arc<CloseCeremonyInterventionUseCase>,
    pub(super) collect_ceremony_evidence: Arc<CollectCeremonyEvidenceUseCase>,
    pub(super) read_ceremony_events: Arc<ReadCeremonyEventsUseCase>,
    pub(super) stream_ceremony: Arc<StreamCeremonyUseCase>,
    pub(super) pull_ceremony_events: Arc<PullCeremonyEventsUseCase>,
    pub(super) verify_ceremony_journal: Arc<VerifyCeremonyJournalUseCase>,
    pub(super) get_ceremony_transcript: Arc<GetCeremonyTranscriptUseCase>,
    pub(super) generate_ceremony_report: Arc<GenerateCeremonyReportUseCase>,
    pub(super) diff_ceremony_definitions: Arc<DiffCeremonyDefinitionsUseCase>,
    pub(super) bind_ceremony_participants: Arc<BindCeremonyParticipantsUseCase>,
    pub(super) publish_ceremony_definition: Arc<PublishCeremonyDefinitionUseCase>,
    pub(super) ceremony_definitions: Arc<dyn CeremonyDefinitionRepositoryPort>,
    pub(super) prepare_ceremony_participants: Arc<PrepareCeremonyParticipantsUseCase>,
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
        instance: &made_core::entities::CeremonyInstance,
    ) -> Result<pb::CeremonyInstanceState, Status> {
        let definition = self
            .resolve_ceremony_definition
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
        instance: &made_core::entities::CeremonyInstance,
        definition: &made_core::entities::CeremonyDefinition,
    ) -> Result<pb::CeremonyInstanceState, Status> {
        let head = self
            .get_ceremony_instance
            .execute_with_ids(instance.id())
            .await
            .map_err(domain_error_to_status)?;
        self.render_read(&head, definition)
            .map_err(domain_error_to_status)
    }

    fn render_read(
        &self,
        read: &made_app::usecases::CeremonyInstanceRead,
        definition: &made_core::entities::CeremonyDefinition,
    ) -> Result<pb::CeremonyInstanceState, made_core::error::DomainError> {
        let view = CeremonyInstanceView::project_at(
            read.instance(),
            definition,
            self.clock.now(),
            self.max_parallel_ceiling,
        )?;
        let mut state = ceremony_instance_state_from(&view);
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
