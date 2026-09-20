//! Every port a service is composed from, one setter each.
//!
//! Its own file so the assembly next door stays readable: the list of
//! what a service can be given grows with every capability, and a file
//! that held both the list and the assembly would be one file nobody
//! reads and everybody edits.

use std::sync::Arc;

use made_app::artifacts::ArtifactService;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, ContinueAcceptedStepClaimUseCase,
    ReadAuthorizationDecisionsUseCase, ReadAuthorizationPolicyUseCase,
};
use made_app::budgets::{
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedCeremonyUseCase,
};
use made_app::services::AutoDispatchService;
use made_app::usecases::SearchCeremonyInstancesUseCase;
use made_app::usecases::{
    AcceptChildCompletionUseCase, AcknowledgeCeremonyAgentInterventionUseCase,
    ApplyCeremonyTransitionUseCase, ApproveCeremonyGuardUseCase, AssertCeremonyReasonUseCase,
    BindCeremonyParticipantsUseCase, CancelCeremonyUseCase, CeremonyAgentStatusService,
    CloseCeremonyInterventionUseCase, CollectCeremonyEvidenceUseCase, CompleteCeremonyStepUseCase,
    CreateCouncilUseCase, DeferCeremonyGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffCeremonyDefinitionsUseCase, EnforceCeremonyDeadlinesUseCase, GenerateCeremonyReportUseCase,
    GetCeremonyInstanceUseCase, GetCeremonyInterventionUseCase, GetCeremonyTranscriptUseCase,
    GetDeliberationUseCase, ListCeremonyInstancesUseCase, ListCeremonyInterventionsUseCase,
    ListCouncilsUseCase, OrchestrateUseCase, PauseCeremonyUseCase,
    PrepareCeremonyParticipantsUseCase, PublishCeremonyDefinitionUseCase,
    PullCeremonyAgentInterventionsUseCase, PullCeremonyEventsUseCase, ReadCeremonyEventsUseCase,
    RecoverCeremonyChildrenUseCase, RegisterAgentUseCase, RequestCeremonyInterventionUseCase,
    ResolveCeremonyDefinitionUseCase, RespondToCeremonyInterventionUseCase, ResumeCeremonyUseCase,
    RunCeremonyStepUseCase, RunCeremonyUseCase, RunCouncilDecisionUseCase,
    StartCeremonyStepUseCase, StartCeremonyUseCase, StartPublishedCeremonyUseCase,
    StreamCeremonyUseCase, UnregisterAgentUseCase, VerifyCeremonyJournalUseCase,
};
use made_app::workers::{
    CompleteExecutionReceiptUseCase, GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::ports::{
    CeremonyDefinitionRepositoryPort, ClockPort, ContractRegistryPort, MetricsRecorderPort,
    MetricsSnapshotPort, StatisticsPort,
};
use made_core::value_objects::MaxParallel;

use super::GrpcAuthorizationGate;

use super::MadeGrpcServiceBuilder;

/// The same shape sixty-seven times: take a port, remember it, hand the
/// builder back. Written once so a new capability cannot arrive with a
/// setter that quietly differs from its neighbours.
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
    setter!(authorization, GrpcAuthorizationGate, authorization);
    setter!(
        authorization_administration,
        AuthorizationPolicyAdministrationService,
        authorization_administration
    );
    setter!(
        read_authorization_policy,
        ReadAuthorizationPolicyUseCase,
        read_authorization_policy
    );
    setter!(
        read_authorization_decisions,
        ReadAuthorizationDecisionsUseCase,
        read_authorization_decisions
    );
    setter!(
        continue_accepted_step_claim,
        ContinueAcceptedStepClaimUseCase,
        continue_accepted_step_claim
    );
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
        search_ceremony_instances,
        SearchCeremonyInstancesUseCase,
        search_ceremony_instances
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
        renew_ceremony_step_lease,
        made_app::workers::RenewCeremonyStepLeaseUseCase,
        renew_ceremony_step_lease
    );
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
    setter!(
        record_ceremony_host_handoff,
        made_app::workers::RecordCeremonyHostHandoffUseCase,
        record_ceremony_host_handoff
    );
    setter!(
        inspect_ceremony_resume,
        made_app::workers::InspectCeremonyResumeUseCase,
        inspect_ceremony_resume
    );
    setter!(
        plan_ceremony_successor,
        made_app::usecases::PlanCeremonySuccessorUseCase,
        plan_ceremony_successor
    );
    setter!(
        start_ceremony_successor,
        made_app::usecases::StartCeremonySuccessorUseCase,
        start_ceremony_successor
    );
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
        pull_ceremony_agent_interventions,
        PullCeremonyAgentInterventionsUseCase,
        pull_ceremony_agent_interventions
    );
    setter!(
        acknowledge_ceremony_agent_intervention,
        AcknowledgeCeremonyAgentInterventionUseCase,
        acknowledge_ceremony_agent_intervention
    );
    setter!(
        get_ceremony_intervention,
        GetCeremonyInterventionUseCase,
        get_ceremony_intervention
    );
    setter!(
        list_ceremony_interventions,
        ListCeremonyInterventionsUseCase,
        list_ceremony_interventions
    );
    setter!(
        collect_ceremony_evidence,
        CollectCeremonyEvidenceUseCase,
        collect_ceremony_evidence
    );
    setter!(
        ceremony_agent_status,
        CeremonyAgentStatusService,
        ceremony_agent_status
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
}
