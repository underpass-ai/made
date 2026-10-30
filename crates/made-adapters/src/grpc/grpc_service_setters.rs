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
    BudgetLedgerService, BudgetedStepClaimUseCase, StartBudgetedProtocolUseCase,
};
use made_app::services::AutoDispatchService;
use made_app::usecases::{
    AcceptChildCompletionUseCase, AcknowledgeProtocolAgentInterventionUseCase,
    ApplyProtocolTransitionUseCase, ApproveProtocolGuardUseCase, AssertProtocolReasonUseCase,
    BindProtocolParticipantsUseCase, CancelProtocolUseCase, CloseProtocolInterventionUseCase,
    CollectProtocolEvidenceUseCase, CompleteProtocolStepUseCase, CreateCouncilUseCase,
    DeferProtocolGuardUseCase, DeleteCouncilUseCase, DeliberateUseCase,
    DiffProtocolDefinitionsUseCase, EnforceProtocolDeadlinesUseCase, GenerateProtocolReportUseCase,
    GetDeliberationUseCase, GetProtocolInstanceUseCase, GetProtocolInterventionUseCase,
    GetProtocolTranscriptUseCase, ListCouncilsUseCase, ListProtocolInstancesUseCase,
    ListProtocolInterventionsUseCase, OrchestrateUseCase, PauseProtocolUseCase,
    PrepareProtocolParticipantsUseCase, ProtocolAgentStatusService,
    PublishProtocolDefinitionUseCase, PullProtocolAgentInterventionsUseCase,
    PullProtocolEventsUseCase, ReadProtocolEventsUseCase, RecoverProtocolChildrenUseCase,
    RegisterAgentUseCase, RequestProtocolInterventionUseCase, ResolveProtocolDefinitionUseCase,
    RespondToProtocolInterventionUseCase, ResumeProtocolUseCase, RunCouncilDecisionUseCase,
    RunProtocolStepUseCase, RunProtocolUseCase, SearchProtocolInstancesUseCase,
    StartProtocolStepUseCase, StartProtocolUseCase, StartPublishedProtocolUseCase,
    StreamProtocolUseCase, UnregisterAgentUseCase, VerifyProtocolJournalUseCase,
};
use made_app::workers::{
    CompleteExecutionReceiptUseCase, GetExecutionReceiptUseCase, InspectExecutionRecoveryUseCase,
};
use made_core::ports::{
    ClockPort, ContractRegistryPort, MetricsRecorderPort, MetricsSnapshotPort,
    ProtocolDefinitionRepositoryPort, StatisticsPort,
};
use made_core::value_objects::MaxParallel;

use super::GrpcAuthorizationGate;

use super::MadeGrpcServiceBuilder;

/// The same shape for every port: take one, remember it, hand the
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
    setter!(run_protocol, RunProtocolUseCase, run_protocol);
    setter!(
        get_protocol_instance,
        GetProtocolInstanceUseCase,
        get_protocol_instance
    );
    setter!(
        list_protocol_instances,
        ListProtocolInstancesUseCase,
        list_protocol_instances
    );
    setter!(
        search_protocol_instances,
        SearchProtocolInstancesUseCase,
        search_protocol_instances
    );
    setter!(
        resolve_protocol_definition,
        ResolveProtocolDefinitionUseCase,
        resolve_protocol_definition
    );
    setter!(start_protocol, StartProtocolUseCase, start_protocol);
    setter!(
        start_published_protocol,
        StartPublishedProtocolUseCase,
        start_published_protocol
    );
    setter!(run_protocol_step, RunProtocolStepUseCase, run_protocol_step);
    setter!(
        renew_protocol_step_lease,
        made_app::workers::RenewProtocolStepLeaseUseCase,
        renew_protocol_step_lease
    );
    setter!(
        accept_child_completion,
        AcceptChildCompletionUseCase,
        accept_child_completion
    );
    setter!(
        recover_protocol_children,
        RecoverProtocolChildrenUseCase,
        recover_protocol_children
    );
    setter!(
        claim_protocol_step,
        StartProtocolStepUseCase,
        claim_protocol_step
    );
    setter!(
        complete_protocol_step,
        CompleteProtocolStepUseCase,
        complete_protocol_step
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
        apply_protocol_transition,
        ApplyProtocolTransitionUseCase,
        apply_protocol_transition
    );
    setter!(pause_protocol, PauseProtocolUseCase, pause_protocol);
    setter!(
        agentic_system,
        crate::grpc::AgenticSystemOperations,
        agentic_system
    );
    setter!(
        record_protocol_host_handoff,
        made_app::workers::RecordProtocolHostHandoffUseCase,
        record_protocol_host_handoff
    );
    setter!(
        inspect_protocol_resume,
        made_app::workers::InspectProtocolResumeUseCase,
        inspect_protocol_resume
    );
    setter!(
        plan_protocol_successor,
        made_app::usecases::PlanProtocolSuccessorUseCase,
        plan_protocol_successor
    );
    setter!(
        start_protocol_successor,
        made_app::usecases::StartProtocolSuccessorUseCase,
        start_protocol_successor
    );
    setter!(resume_protocol, ResumeProtocolUseCase, resume_protocol);
    setter!(cancel_protocol, CancelProtocolUseCase, cancel_protocol);
    setter!(
        enforce_protocol_deadlines,
        EnforceProtocolDeadlinesUseCase,
        enforce_protocol_deadlines
    );
    setter!(
        approve_protocol_guard,
        ApproveProtocolGuardUseCase,
        approve_protocol_guard
    );
    setter!(
        defer_protocol_guard,
        DeferProtocolGuardUseCase,
        defer_protocol_guard
    );
    setter!(
        assert_protocol_reason,
        AssertProtocolReasonUseCase,
        assert_protocol_reason
    );
    setter!(
        request_protocol_intervention,
        RequestProtocolInterventionUseCase,
        request_protocol_intervention
    );
    setter!(
        respond_to_protocol_intervention,
        RespondToProtocolInterventionUseCase,
        respond_to_protocol_intervention
    );
    setter!(
        close_protocol_intervention,
        CloseProtocolInterventionUseCase,
        close_protocol_intervention
    );
    setter!(
        collect_protocol_evidence,
        CollectProtocolEvidenceUseCase,
        collect_protocol_evidence
    );
    setter!(
        protocol_agent_status,
        ProtocolAgentStatusService,
        protocol_agent_status
    );
    setter!(
        read_protocol_events,
        ReadProtocolEventsUseCase,
        read_protocol_events
    );
    setter!(stream_protocol, StreamProtocolUseCase, stream_protocol);
    setter!(
        pull_protocol_events,
        PullProtocolEventsUseCase,
        pull_protocol_events
    );
    setter!(
        verify_protocol_journal,
        VerifyProtocolJournalUseCase,
        verify_protocol_journal
    );
    setter!(
        get_protocol_transcript,
        GetProtocolTranscriptUseCase,
        get_protocol_transcript
    );
    setter!(
        generate_protocol_report,
        GenerateProtocolReportUseCase,
        generate_protocol_report
    );
    setter!(
        prepare_protocol_participants,
        PrepareProtocolParticipantsUseCase,
        prepare_protocol_participants
    );
    setter!(
        bind_protocol_participants,
        BindProtocolParticipantsUseCase,
        bind_protocol_participants
    );
    setter!(
        diff_protocol_definitions,
        DiffProtocolDefinitionsUseCase,
        diff_protocol_definitions
    );
    setter!(
        publish_protocol_definition,
        PublishProtocolDefinitionUseCase,
        publish_protocol_definition
    );
    setter!(auto_dispatch, AutoDispatchService, auto_dispatch);
    setter!(artifacts, ArtifactService, artifacts);
    setter!(budgets, BudgetLedgerService, budgets);
    setter!(
        start_budgeted_protocol,
        StartBudgetedProtocolUseCase,
        start_budgeted_protocol
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
    pub fn protocol_definitions(
        mut self,
        value: Arc<dyn ProtocolDefinitionRepositoryPort>,
    ) -> Self {
        self.protocol_definitions = Some(value);
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
    setter!(
        pull_protocol_agent_interventions,
        PullProtocolAgentInterventionsUseCase,
        pull_protocol_agent_interventions
    );
    setter!(
        acknowledge_protocol_agent_intervention,
        AcknowledgeProtocolAgentInterventionUseCase,
        acknowledge_protocol_agent_intervention
    );
    setter!(
        get_protocol_intervention,
        GetProtocolInterventionUseCase,
        get_protocol_intervention
    );
    setter!(
        list_protocol_interventions,
        ListProtocolInterventionsUseCase,
        list_protocol_interventions
    );
    setter!(
        integrator_loop,
        crate::grpc::service::integrator_loop_operations::IntegratorLoopOperations,
        integrator_loop
    );
}
