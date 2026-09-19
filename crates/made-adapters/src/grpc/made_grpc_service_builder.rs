use std::sync::Arc;

use made_app::artifacts::ArtifactService;
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedCeremonyUseCase,
};
use made_app::services::AutoDispatchService;
use made_app::usecases::{
    AcceptChildCompletionUseCase, ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase,
    AssertCeremonyReasonUseCase, BindCeremonyParticipantsUseCase, CancelCeremonyUseCase,
    CloseCeremonyInterventionUseCase, CollectCeremonyEvidenceUseCase, CompleteCeremonyStepUseCase,
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
use made_core::ports::{
    CeremonyDefinitionRepositoryPort, ClockPort, ContractRegistryPort, MetricsRecorderPort,
    MetricsSnapshotPort, NoopMetricsRecorder, NoopMetricsSnapshot, StatisticsPort,
};
use made_core::value_objects::MaxParallel;

/// Builder so composition-root wiring is readable even as the number
/// of use cases grows.
#[derive(Default)]
pub struct MadeGrpcServiceBuilder {
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
    pub(super) run_ceremony: Option<Arc<RunCeremonyUseCase>>,
    pub(super) get_ceremony_instance: Option<Arc<GetCeremonyInstanceUseCase>>,
    pub(super) list_ceremony_instances: Option<Arc<ListCeremonyInstancesUseCase>>,
    pub(super) resolve_ceremony_definition: Option<Arc<ResolveCeremonyDefinitionUseCase>>,
    pub(super) start_ceremony: Option<Arc<StartCeremonyUseCase>>,
    pub(super) start_published_ceremony: Option<Arc<StartPublishedCeremonyUseCase>>,
    pub(super) start_budgeted_ceremony: Option<Arc<StartBudgetedCeremonyUseCase>>,
    pub(super) run_ceremony_step: Option<Arc<RunCeremonyStepUseCase>>,
    pub(super) accept_child_completion: Option<Arc<AcceptChildCompletionUseCase>>,
    pub(super) recover_ceremony_children: Option<Arc<RecoverCeremonyChildrenUseCase>>,
    pub(super) claim_ceremony_step: Option<Arc<StartCeremonyStepUseCase>>,
    pub(super) budgeted_step_claim: Option<Arc<BudgetedStepClaimUseCase>>,
    pub(super) complete_ceremony_step: Option<Arc<CompleteCeremonyStepUseCase>>,
    pub(super) get_execution_receipt: Option<Arc<GetExecutionReceiptUseCase>>,
    pub(super) inspect_execution_recovery: Option<Arc<InspectExecutionRecoveryUseCase>>,
    pub(super) complete_execution_receipt: Option<Arc<CompleteExecutionReceiptUseCase>>,
    pub(super) apply_ceremony_transition: Option<Arc<ApplyCeremonyTransitionUseCase>>,
    pub(super) pause_ceremony: Option<Arc<PauseCeremonyUseCase>>,
    pub(super) resume_ceremony: Option<Arc<ResumeCeremonyUseCase>>,
    pub(super) cancel_ceremony: Option<Arc<CancelCeremonyUseCase>>,
    pub(super) enforce_ceremony_deadlines: Option<Arc<EnforceCeremonyDeadlinesUseCase>>,
    pub(super) approve_ceremony_guard: Option<Arc<ApproveCeremonyGuardUseCase>>,
    pub(super) defer_ceremony_guard: Option<Arc<DeferCeremonyGuardUseCase>>,
    pub(super) assert_ceremony_reason: Option<Arc<AssertCeremonyReasonUseCase>>,
    pub(super) request_ceremony_intervention: Option<Arc<RequestCeremonyInterventionUseCase>>,
    pub(super) respond_to_ceremony_intervention: Option<Arc<RespondToCeremonyInterventionUseCase>>,
    pub(super) close_ceremony_intervention: Option<Arc<CloseCeremonyInterventionUseCase>>,
    pub(super) collect_ceremony_evidence: Option<Arc<CollectCeremonyEvidenceUseCase>>,
    pub(super) read_ceremony_events: Option<Arc<ReadCeremonyEventsUseCase>>,
    pub(super) stream_ceremony: Option<Arc<StreamCeremonyUseCase>>,
    pub(super) pull_ceremony_events: Option<Arc<PullCeremonyEventsUseCase>>,
    pub(super) verify_ceremony_journal: Option<Arc<VerifyCeremonyJournalUseCase>>,
    pub(super) get_ceremony_transcript: Option<Arc<GetCeremonyTranscriptUseCase>>,
    pub(super) generate_ceremony_report: Option<Arc<GenerateCeremonyReportUseCase>>,
    pub(super) diff_ceremony_definitions: Option<Arc<DiffCeremonyDefinitionsUseCase>>,
    pub(super) bind_ceremony_participants: Option<Arc<BindCeremonyParticipantsUseCase>>,
    pub(super) publish_ceremony_definition: Option<Arc<PublishCeremonyDefinitionUseCase>>,
    pub(super) ceremony_definitions: Option<Arc<dyn CeremonyDefinitionRepositoryPort>>,
    pub(super) prepare_ceremony_participants: Option<Arc<PrepareCeremonyParticipantsUseCase>>,
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

macro_rules! setter {
    ($name:ident, $ty:ty, $field:ident) => {
        #[must_use]
        pub fn $name(mut self, value: Arc<$ty>) -> Self {
            self.$field = Some(value);
            self
        }
    };
}

impl MadeGrpcServiceBuilder {
    setter!(
        council_journal,
        made_app::services::CouncilJournalService,
        council_journal
    );
    setter!(deliberate, DeliberateUseCase, deliberate);
    setter!(orchestrate, OrchestrateUseCase, orchestrate);
    setter!(create_council, CreateCouncilUseCase, create_council);
    setter!(delete_council, DeleteCouncilUseCase, delete_council);
    setter!(list_councils, ListCouncilsUseCase, list_councils);
    setter!(get_deliberation, GetDeliberationUseCase, get_deliberation);
    setter!(register_agent, RegisterAgentUseCase, register_agent);
    setter!(unregister_agent, UnregisterAgentUseCase, unregister_agent);
    setter!(
        run_council_decision,
        RunCouncilDecisionUseCase,
        run_council_decision
    );
    setter!(run_ceremony, RunCeremonyUseCase, run_ceremony);
    setter!(
        get_ceremony_instance,
        GetCeremonyInstanceUseCase,
        get_ceremony_instance
    );
    setter!(
        list_ceremony_instances,
        ListCeremonyInstancesUseCase,
        list_ceremony_instances
    );
    setter!(
        resolve_ceremony_definition,
        ResolveCeremonyDefinitionUseCase,
        resolve_ceremony_definition
    );
    setter!(start_ceremony, StartCeremonyUseCase, start_ceremony);
    setter!(
        start_published_ceremony,
        StartPublishedCeremonyUseCase,
        start_published_ceremony
    );
    setter!(run_ceremony_step, RunCeremonyStepUseCase, run_ceremony_step);
    setter!(
        accept_child_completion,
        AcceptChildCompletionUseCase,
        accept_child_completion
    );
    setter!(
        recover_ceremony_children,
        RecoverCeremonyChildrenUseCase,
        recover_ceremony_children
    );
    setter!(
        claim_ceremony_step,
        StartCeremonyStepUseCase,
        claim_ceremony_step
    );
    setter!(
        complete_ceremony_step,
        CompleteCeremonyStepUseCase,
        complete_ceremony_step
    );
    setter!(
        get_execution_receipt,
        GetExecutionReceiptUseCase,
        get_execution_receipt
    );
    setter!(
        inspect_execution_recovery,
        InspectExecutionRecoveryUseCase,
        inspect_execution_recovery
    );
    setter!(
        complete_execution_receipt,
        CompleteExecutionReceiptUseCase,
        complete_execution_receipt
    );
    setter!(
        apply_ceremony_transition,
        ApplyCeremonyTransitionUseCase,
        apply_ceremony_transition
    );
    setter!(pause_ceremony, PauseCeremonyUseCase, pause_ceremony);
    setter!(resume_ceremony, ResumeCeremonyUseCase, resume_ceremony);
    setter!(cancel_ceremony, CancelCeremonyUseCase, cancel_ceremony);
    setter!(
        enforce_ceremony_deadlines,
        EnforceCeremonyDeadlinesUseCase,
        enforce_ceremony_deadlines
    );
    setter!(
        approve_ceremony_guard,
        ApproveCeremonyGuardUseCase,
        approve_ceremony_guard
    );
    setter!(
        defer_ceremony_guard,
        DeferCeremonyGuardUseCase,
        defer_ceremony_guard
    );
    setter!(
        assert_ceremony_reason,
        AssertCeremonyReasonUseCase,
        assert_ceremony_reason
    );
    setter!(
        request_ceremony_intervention,
        RequestCeremonyInterventionUseCase,
        request_ceremony_intervention
    );
    setter!(
        respond_to_ceremony_intervention,
        RespondToCeremonyInterventionUseCase,
        respond_to_ceremony_intervention
    );
    setter!(
        close_ceremony_intervention,
        CloseCeremonyInterventionUseCase,
        close_ceremony_intervention
    );
    setter!(
        collect_ceremony_evidence,
        CollectCeremonyEvidenceUseCase,
        collect_ceremony_evidence
    );
    setter!(
        read_ceremony_events,
        ReadCeremonyEventsUseCase,
        read_ceremony_events
    );
    setter!(stream_ceremony, StreamCeremonyUseCase, stream_ceremony);
    setter!(
        pull_ceremony_events,
        PullCeremonyEventsUseCase,
        pull_ceremony_events
    );
    setter!(
        verify_ceremony_journal,
        VerifyCeremonyJournalUseCase,
        verify_ceremony_journal
    );
    setter!(
        get_ceremony_transcript,
        GetCeremonyTranscriptUseCase,
        get_ceremony_transcript
    );
    setter!(
        generate_ceremony_report,
        GenerateCeremonyReportUseCase,
        generate_ceremony_report
    );
    setter!(
        prepare_ceremony_participants,
        PrepareCeremonyParticipantsUseCase,
        prepare_ceremony_participants
    );
    setter!(
        bind_ceremony_participants,
        BindCeremonyParticipantsUseCase,
        bind_ceremony_participants
    );
    setter!(
        diff_ceremony_definitions,
        DiffCeremonyDefinitionsUseCase,
        diff_ceremony_definitions
    );
    setter!(
        publish_ceremony_definition,
        PublishCeremonyDefinitionUseCase,
        publish_ceremony_definition
    );
    setter!(auto_dispatch, AutoDispatchService, auto_dispatch);
    setter!(artifacts, ArtifactService, artifacts);
    setter!(budgets, BudgetLedgerService, budgets);
    setter!(
        start_budgeted_ceremony,
        StartBudgetedCeremonyUseCase,
        start_budgeted_ceremony
    );
    setter!(
        budgeted_step_claim,
        BudgetedStepClaimUseCase,
        budgeted_step_claim
    );

    #[must_use]
    pub fn statistics(mut self, value: Arc<dyn StatisticsPort>) -> Self {
        self.statistics = Some(value);
        self
    }

    #[must_use]
    pub fn ceremony_definitions(
        mut self,
        value: Arc<dyn CeremonyDefinitionRepositoryPort>,
    ) -> Self {
        self.ceremony_definitions = Some(value);
        self
    }

    #[must_use]
    pub fn contract_registry(mut self, value: Arc<dyn ContractRegistryPort>) -> Self {
        self.contract_registry = Some(value);
        self
    }

    /// Configure only the operational-metrics write side. Prefer
    /// [`Self::observability`] when the adapter also serves registry reads.
    #[must_use]
    pub fn metrics(mut self, value: Arc<dyn MetricsRecorderPort>) -> Self {
        self.metrics = Some(value);
        self
    }

    /// Read operational metrics from a separately supplied registry.
    /// Prefer [`Self::observability`] when one adapter serves both sides.
    #[must_use]
    pub fn metrics_snapshot(mut self, value: Arc<dyn MetricsSnapshotPort>) -> Self {
        self.metrics_snapshot = Some(value);
        self
    }

    /// Record and read operational metrics through the same adapter instance.
    #[must_use]
    pub fn observability<M>(mut self, value: Arc<M>) -> Self
    where
        M: MetricsRecorderPort + MetricsSnapshotPort + 'static,
    {
        self.metrics = Some(value.clone());
        self.metrics_snapshot = Some(value);
        self
    }

    #[must_use]
    pub fn service_version(mut self, value: &'static str) -> Self {
        self.service_version = Some(value);
        self
    }

    #[must_use]
    pub fn clock(mut self, value: Arc<dyn ClockPort>) -> Self {
        self.clock = Some(value);
        self
    }

    #[must_use]
    pub fn max_parallel_ceiling(mut self, value: MaxParallel) -> Self {
        self.max_parallel_ceiling = Some(value);
        self
    }

    /// Consume the builder. Missing dependencies are reported via
    /// [`DomainError::InvariantViolated`] so wiring errors surface
    /// through the same error channel the rest of the app uses.
    pub fn build(self) -> Result<MadeGrpcService, DomainError> {
        // Composed here rather than in a handler: the uptime clock
        // starts when the service is built, and what a status *is*
        // belongs to the use case both editions call.
        let statistics = required!(self, statistics, "port");
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
        Ok(MadeGrpcService {
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
            run_ceremony: required!(self, run_ceremony),
            get_ceremony_instance: required!(self, get_ceremony_instance),
            list_ceremony_instances: required!(self, list_ceremony_instances),
            resolve_ceremony_definition: required!(self, resolve_ceremony_definition),
            start_ceremony: required!(self, start_ceremony),
            start_published_ceremony: required!(self, start_published_ceremony),
            start_budgeted_ceremony: self.start_budgeted_ceremony,
            run_ceremony_step: required!(self, run_ceremony_step),
            accept_child_completion: required!(self, accept_child_completion),
            recover_ceremony_children: required!(self, recover_ceremony_children),
            claim_ceremony_step: required!(self, claim_ceremony_step),
            budgeted_step_claim: self.budgeted_step_claim,
            complete_ceremony_step: required!(self, complete_ceremony_step),
            get_execution_receipt: self.get_execution_receipt,
            inspect_execution_recovery: self.inspect_execution_recovery,
            complete_execution_receipt: self.complete_execution_receipt,
            apply_ceremony_transition: required!(self, apply_ceremony_transition),
            pause_ceremony: required!(self, pause_ceremony),
            resume_ceremony: required!(self, resume_ceremony),
            cancel_ceremony: required!(self, cancel_ceremony),
            enforce_ceremony_deadlines: required!(self, enforce_ceremony_deadlines),
            approve_ceremony_guard: required!(self, approve_ceremony_guard),
            defer_ceremony_guard: required!(self, defer_ceremony_guard),
            assert_ceremony_reason: required!(self, assert_ceremony_reason),
            request_ceremony_intervention: required!(self, request_ceremony_intervention),
            respond_to_ceremony_intervention: required!(self, respond_to_ceremony_intervention),
            close_ceremony_intervention: required!(self, close_ceremony_intervention),
            collect_ceremony_evidence: required!(self, collect_ceremony_evidence),
            read_ceremony_events: required!(self, read_ceremony_events),
            stream_ceremony: required!(self, stream_ceremony),
            pull_ceremony_events: required!(self, pull_ceremony_events),
            verify_ceremony_journal: required!(self, verify_ceremony_journal),
            get_ceremony_transcript: required!(self, get_ceremony_transcript),
            generate_ceremony_report: required!(self, generate_ceremony_report),
            publish_ceremony_definition: required!(self, publish_ceremony_definition),
            diff_ceremony_definitions: required!(self, diff_ceremony_definitions),
            bind_ceremony_participants: required!(self, bind_ceremony_participants),
            ceremony_definitions: required!(self, ceremony_definitions, "port"),
            prepare_ceremony_participants: required!(self, prepare_ceremony_participants),
            contract_registry: required!(self, contract_registry, "port"),
            auto_dispatch: required!(self, auto_dispatch, "service"),
            get_service_status,
            get_service_metrics,
            artifacts: self.artifacts,
            budgets: self.budgets,
        })
    }
}
