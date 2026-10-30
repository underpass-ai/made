use std::sync::Arc;

use made_app::artifacts::ArtifactService;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, ContinueAcceptedStepClaimUseCase,
    ReadAuthorizationDecisionsUseCase, ReadAuthorizationPolicyUseCase,
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
    CollectProtocolEvidenceUseCase, CompleteProtocolStepUseCase, CreateCouncilUseCase,
    DeferProtocolGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffProtocolDefinitionsUseCase, EnforceProtocolDeadlinesUseCase, GenerateProtocolReportUseCase,
    GetDeliberationUseCase, GetProtocolInstanceUseCase, GetProtocolInterventionUseCase,
    GetProtocolTranscriptUseCase, GetServiceMetricsUseCase, GetServiceStatusUseCase,
    ListCouncilsUseCase, ListProtocolInstancesUseCase, ListProtocolInterventionsUseCase,
    OrchestrateUseCase, PauseProtocolUseCase, PrepareProtocolParticipantsUseCase,
    ProtocolAgentStatusService, PublishProtocolDefinitionUseCase,
    PullProtocolAgentInterventionsUseCase, PullProtocolEventsUseCase, ReadProtocolEventsUseCase,
    RecoverProtocolChildrenUseCase, RegisterAgentUseCase, RequestProtocolInterventionUseCase,
    ResolveProtocolDefinitionUseCase, RespondToProtocolInterventionUseCase, ResumeProtocolUseCase,
    RunCouncilDecisionUseCase, RunProtocolStepUseCase, RunProtocolUseCase,
    StartProtocolStepUseCase, StartProtocolUseCase, StartPublishedProtocolUseCase,
    StreamProtocolUseCase, UnregisterAgentUseCase, VerifyProtocolJournalUseCase,
};
use made_app::workers::{
    CompleteExecutionReceiptUseCase, GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::ports::{
    ClockPort, ContractRegistryPort, MetricsRecorderPort, MetricsSnapshotPort, NoopMetricsRecorder,
    NoopMetricsSnapshot, ProtocolDefinitionPublicationPort, ProtocolDefinitionRepositoryPort,
    StatisticsPort,
};
use made_core::value_objects::MaxParallel;

use super::GrpcAuthorizationGate;

/// Builder so composition-root wiring is readable even as the number
/// of use cases grows.
#[derive(Default)]
pub struct MadeGrpcServiceBuilder {
    pub(super) agentic_system: Option<Arc<crate::grpc::AgenticSystemOperations>>,
    pub(super) record_protocol_host_handoff:
        Option<Arc<made_app::workers::RecordProtocolHostHandoffUseCase>>,
    pub(super) inspect_protocol_resume:
        Option<Arc<made_app::workers::InspectProtocolResumeUseCase>>,
    pub(super) plan_protocol_successor:
        Option<Arc<made_app::usecases::PlanProtocolSuccessorUseCase>>,
    pub(super) start_protocol_successor:
        Option<Arc<made_app::usecases::StartProtocolSuccessorUseCase>>,
    pub(super) authorization: Option<Arc<GrpcAuthorizationGate>>,
    pub(super) authorization_administration: Option<Arc<AuthorizationPolicyAdministrationService>>,
    pub(super) read_authorization_policy: Option<Arc<ReadAuthorizationPolicyUseCase>>,
    pub(super) read_authorization_decisions: Option<Arc<ReadAuthorizationDecisionsUseCase>>,
    pub(super) continue_accepted_step_claim: Option<Arc<ContinueAcceptedStepClaimUseCase>>,
    pub(super) council_journal: Option<Arc<made_app::services::CouncilJournalService>>,
    pub(super) deliberate: Option<Arc<DeliberateUseCase>>,
    pub(super) orchestrate: Option<Arc<OrchestrateUseCase>>,
    pub(super) create_council: Option<Arc<CreateCouncilUseCase>>,
    pub(super) delete_council: Option<Arc<DeleteCouncilUseCase>>,
    pub(super) list_councils: Option<Arc<ListCouncilsUseCase>>,
    pub(super) get_deliberation: Option<Arc<GetDeliberationUseCase>>,
    pub(super) register_agent: Option<Arc<RegisterAgentUseCase>>,
    pub(super) unregister_agent: Option<Arc<UnregisterAgentUseCase>>,
    pub(super) run_council_decision: Option<Arc<RunCouncilDecisionUseCase>>,
    pub(super) run_protocol: Option<Arc<RunProtocolUseCase>>,
    pub(super) get_protocol_instance: Option<Arc<GetProtocolInstanceUseCase>>,
    pub(super) list_protocol_instances: Option<Arc<ListProtocolInstancesUseCase>>,
    pub(super) search_protocol_instances: Option<Arc<SearchProtocolInstancesUseCase>>,
    pub(super) resolve_protocol_definition: Option<Arc<ResolveProtocolDefinitionUseCase>>,
    pub(super) start_protocol: Option<Arc<StartProtocolUseCase>>,
    pub(super) start_published_protocol: Option<Arc<StartPublishedProtocolUseCase>>,
    pub(super) start_budgeted_protocol: Option<Arc<StartBudgetedProtocolUseCase>>,
    pub(super) run_protocol_step: Option<Arc<RunProtocolStepUseCase>>,
    pub(super) accept_child_completion: Option<Arc<AcceptChildCompletionUseCase>>,
    pub(super) recover_protocol_children: Option<Arc<RecoverProtocolChildrenUseCase>>,
    pub(super) claim_protocol_step: Option<Arc<StartProtocolStepUseCase>>,
    pub(super) budgeted_step_claim: Option<Arc<BudgetedStepClaimUseCase>>,
    pub(super) complete_protocol_step: Option<Arc<CompleteProtocolStepUseCase>>,
    pub(super) renew_protocol_step_lease:
        Option<Arc<made_app::workers::RenewProtocolStepLeaseUseCase>>,
    pub(super) get_execution_receipt: Option<Arc<GetExecutionReceiptUseCase>>,
    pub(super) inspect_execution_recovery: Option<Arc<InspectExecutionRecoveryUseCase>>,
    pub(super) complete_execution_receipt: Option<Arc<CompleteExecutionReceiptUseCase>>,
    pub(super) apply_protocol_transition: Option<Arc<ApplyProtocolTransitionUseCase>>,
    pub(super) pause_protocol: Option<Arc<PauseProtocolUseCase>>,
    pub(super) resume_protocol: Option<Arc<ResumeProtocolUseCase>>,
    pub(super) cancel_protocol: Option<Arc<CancelProtocolUseCase>>,
    pub(super) enforce_protocol_deadlines: Option<Arc<EnforceProtocolDeadlinesUseCase>>,
    pub(super) approve_protocol_guard: Option<Arc<ApproveProtocolGuardUseCase>>,
    pub(super) defer_protocol_guard: Option<Arc<DeferProtocolGuardUseCase>>,
    pub(super) assert_protocol_reason: Option<Arc<AssertProtocolReasonUseCase>>,
    pub(super) request_protocol_intervention: Option<Arc<RequestProtocolInterventionUseCase>>,
    pub(super) respond_to_protocol_intervention: Option<Arc<RespondToProtocolInterventionUseCase>>,
    pub(super) close_protocol_intervention: Option<Arc<CloseProtocolInterventionUseCase>>,
    pub(super) pull_protocol_agent_interventions:
        Option<Arc<PullProtocolAgentInterventionsUseCase>>,
    pub(super) acknowledge_protocol_agent_intervention:
        Option<Arc<AcknowledgeProtocolAgentInterventionUseCase>>,
    pub(super) get_protocol_intervention: Option<Arc<GetProtocolInterventionUseCase>>,
    pub(super) list_protocol_interventions: Option<Arc<ListProtocolInterventionsUseCase>>,
    pub(super) integrator_loop:
        Option<Arc<crate::grpc::service::integrator_loop_operations::IntegratorLoopOperations>>,
    pub(super) collect_protocol_evidence: Option<Arc<CollectProtocolEvidenceUseCase>>,
    pub(super) protocol_agent_status: Option<Arc<ProtocolAgentStatusService>>,
    pub(super) read_protocol_events: Option<Arc<ReadProtocolEventsUseCase>>,
    pub(super) stream_protocol: Option<Arc<StreamProtocolUseCase>>,
    pub(super) pull_protocol_events: Option<Arc<PullProtocolEventsUseCase>>,
    pub(super) verify_protocol_journal: Option<Arc<VerifyProtocolJournalUseCase>>,
    pub(super) get_protocol_transcript: Option<Arc<GetProtocolTranscriptUseCase>>,
    pub(super) generate_protocol_report: Option<Arc<GenerateProtocolReportUseCase>>,
    pub(super) diff_protocol_definitions: Option<Arc<DiffProtocolDefinitionsUseCase>>,
    /// The published catalogue the two catalogue reads answer from.
    pub(super) protocol_publications: Option<Arc<dyn ProtocolDefinitionPublicationPort>>,
    pub(super) bind_protocol_participants: Option<Arc<BindProtocolParticipantsUseCase>>,
    pub(super) publish_protocol_definition: Option<Arc<PublishProtocolDefinitionUseCase>>,
    pub(super) protocol_definitions: Option<Arc<dyn ProtocolDefinitionRepositoryPort>>,
    pub(super) prepare_protocol_participants: Option<Arc<PrepareProtocolParticipantsUseCase>>,
    pub(super) contract_registry: Option<Arc<dyn ContractRegistryPort>>,
    pub(super) auto_dispatch: Option<Arc<AutoDispatchService>>,
    pub(super) statistics: Option<Arc<dyn StatisticsPort>>,
    /// What records operational metrics in this process.
    ///
    /// Optional because a composition that wires none is still a
    /// running service; it then answers `noop` when asked what is
    /// recording, which is the true answer rather than silence.
    pub(super) metrics: Option<Arc<dyn MetricsRecorderPort>>,
    pub(super) metrics_snapshot: Option<Arc<dyn MetricsSnapshotPort>>,
    pub(super) service_version: Option<&'static str>,
    pub(super) clock: Option<Arc<dyn ClockPort>>,
    pub(super) max_parallel_ceiling: Option<MaxParallel>,
    pub(super) artifacts: Option<Arc<ArtifactService>>,
    pub(super) budgets: Option<Arc<BudgetLedgerService>>,
}

use made_core::error::DomainError;

use super::MadeGrpcService;

impl std::fmt::Debug for MadeGrpcServiceBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MadeGrpcServiceBuilder").finish()
    }
}

macro_rules! required {
    ($self:ident, $field:ident) => {
        required!($self, $field, "use case")
    };
    ($self:ident, $field:ident, $what:literal) => {
        $self.$field.ok_or(DomainError::InvariantViolated {
            reason: concat!("grpc: ", stringify!($field), " ", $what, " is required"),
        })?
    };
}

impl MadeGrpcServiceBuilder {
    /// The published catalogue `ListProtocolDefinitions` and
    /// `GetProtocolDefinition` read.
    #[must_use]
    pub fn protocol_publications(
        mut self,
        value: Arc<dyn ProtocolDefinitionPublicationPort>,
    ) -> Self {
        self.protocol_publications = Some(value);
        self
    }

    /// Consume the builder. Missing dependencies are reported via
    /// [`DomainError::InvariantViolated`] so wiring errors surface
    /// through the same error channel the rest of the app uses.
    /// One exhaustive assembly of every dependency the service
    /// holds. Splitting it would hide which fields are required and
    /// which are optional behind a second function boundary.
    #[allow(clippy::too_many_lines)]
    pub fn build(self) -> Result<MadeGrpcService, DomainError> {
        // Composed here rather than in a handler: the uptime clock
        // starts when the service is built, and what a status *is*
        // belongs to the use case both editions call.
        let statistics = required!(self, statistics, "port");
        let protocol_publications = required!(self, protocol_publications, "port");
        let metrics = self
            .metrics
            .unwrap_or_else(|| Arc::new(NoopMetricsRecorder) as Arc<dyn MetricsRecorderPort>);
        let clock = self
            .clock
            .unwrap_or_else(|| Arc::new(crate::clock::SystemClock::new()) as Arc<dyn ClockPort>);
        let metrics_snapshot = self
            .metrics_snapshot
            .unwrap_or_else(|| Arc::new(NoopMetricsSnapshot) as Arc<dyn MetricsSnapshotPort>);
        let get_service_status = Arc::new(GetServiceStatusUseCase::new(
            statistics.clone(),
            metrics,
            self.service_version.unwrap_or(""),
            clock.clone(),
        ));
        let get_service_metrics =
            Arc::new(GetServiceMetricsUseCase::new(statistics, metrics_snapshot));
        let council_journal = self.council_journal.unwrap_or_else(|| {
            Arc::new(made_app::services::CouncilJournalService::new(
                Arc::new(crate::memory::InMemoryCouncilJournal::new()),
                clock.clone(),
            ))
        });
        let protocol_agent_status = self.protocol_agent_status.unwrap_or_else(|| {
            // A default service remains fail-closed for reports until the
            // composition root supplies the journal-bound service.
            Arc::new(ProtocolAgentStatusService::new(
                Arc::new(crate::memory::InMemoryProtocolAgentStatus::new()),
                clock.clone(),
                time::Duration::seconds(60),
            ))
        });
        Ok(MadeGrpcService {
            agentic_system: self.agentic_system,
            authorization: required!(self, authorization, "gate"),
            authorization_administration: required!(self, authorization_administration),
            read_authorization_policy: required!(self, read_authorization_policy),
            read_authorization_decisions: required!(self, read_authorization_decisions),
            continue_accepted_step_claim: required!(self, continue_accepted_step_claim),
            council_journal,
            clock,
            max_parallel_ceiling: self.max_parallel_ceiling.unwrap_or(MaxParallel::SERVER_MAX),
            deliberate: required!(self, deliberate),
            orchestrate: required!(self, orchestrate),
            create_council: required!(self, create_council),
            delete_council: required!(self, delete_council),
            list_councils: required!(self, list_councils),
            get_deliberation: required!(self, get_deliberation),
            register_agent: required!(self, register_agent),
            unregister_agent: required!(self, unregister_agent),
            run_council_decision: required!(self, run_council_decision),
            run_protocol: required!(self, run_protocol),
            get_protocol_instance: required!(self, get_protocol_instance),
            list_protocol_instances: required!(self, list_protocol_instances),
            search_protocol_instances: required!(self, search_protocol_instances),
            resolve_protocol_definition: required!(self, resolve_protocol_definition),
            start_protocol: required!(self, start_protocol),
            start_published_protocol: required!(self, start_published_protocol),
            start_budgeted_protocol: self.start_budgeted_protocol,
            run_protocol_step: required!(self, run_protocol_step),
            accept_child_completion: required!(self, accept_child_completion),
            recover_protocol_children: required!(self, recover_protocol_children),
            claim_protocol_step: required!(self, claim_protocol_step),
            budgeted_step_claim: self.budgeted_step_claim,
            complete_protocol_step: required!(self, complete_protocol_step),
            renew_protocol_step_lease: self.renew_protocol_step_lease,
            get_execution_receipt: self.get_execution_receipt,
            inspect_execution_recovery: self.inspect_execution_recovery,
            complete_execution_receipt: self.complete_execution_receipt,
            apply_protocol_transition: required!(self, apply_protocol_transition),
            pause_protocol: required!(self, pause_protocol),
            record_protocol_host_handoff: self.record_protocol_host_handoff,
            inspect_protocol_resume: self.inspect_protocol_resume,
            plan_protocol_successor: self.plan_protocol_successor,
            start_protocol_successor: self.start_protocol_successor,
            resume_protocol: required!(self, resume_protocol),
            cancel_protocol: required!(self, cancel_protocol),
            enforce_protocol_deadlines: required!(self, enforce_protocol_deadlines),
            approve_protocol_guard: required!(self, approve_protocol_guard),
            defer_protocol_guard: required!(self, defer_protocol_guard),
            assert_protocol_reason: required!(self, assert_protocol_reason),
            request_protocol_intervention: required!(self, request_protocol_intervention),
            respond_to_protocol_intervention: required!(self, respond_to_protocol_intervention),
            close_protocol_intervention: required!(self, close_protocol_intervention),
            pull_protocol_agent_interventions: required!(self, pull_protocol_agent_interventions),
            acknowledge_protocol_agent_intervention: required!(
                self,
                acknowledge_protocol_agent_intervention
            ),
            get_protocol_intervention: required!(self, get_protocol_intervention),
            list_protocol_interventions: required!(self, list_protocol_interventions),
            integrator_loop: self.integrator_loop,
            collect_protocol_evidence: required!(self, collect_protocol_evidence),
            protocol_agent_status,
            read_protocol_events: required!(self, read_protocol_events),
            stream_protocol: required!(self, stream_protocol),
            pull_protocol_events: required!(self, pull_protocol_events),
            verify_protocol_journal: required!(self, verify_protocol_journal),
            get_protocol_transcript: required!(self, get_protocol_transcript),
            generate_protocol_report: required!(self, generate_protocol_report),
            publish_protocol_definition: required!(self, publish_protocol_definition),
            diff_protocol_definitions: required!(self, diff_protocol_definitions),
            protocol_publications,
            bind_protocol_participants: required!(self, bind_protocol_participants),
            protocol_definitions: required!(self, protocol_definitions, "port"),
            prepare_protocol_participants: required!(self, prepare_protocol_participants),
            contract_registry: required!(self, contract_registry, "port"),
            auto_dispatch: required!(self, auto_dispatch, "service"),
            get_service_status,
            get_service_metrics,
            artifacts: self.artifacts,
            budgets: self.budgets,
        })
    }
}
