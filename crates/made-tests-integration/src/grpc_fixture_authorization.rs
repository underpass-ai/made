use std::sync::Arc;

use made_adapters::grpc::GrpcAuthorizationGate;
use made_adapters::memory::InMemoryAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    ContinueAcceptedCeremonyWorkUseCase, ReadAuthorizationDecisionsUseCase,
    ReadAuthorizationPolicyUseCase,
};
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationScope, DelegationDepth, PrincipalId, PrincipalKind, SeparationRule,
};

pub(crate) struct FixtureAuthorization {
    pub(crate) authorize: Arc<AuthorizeOperationUseCase>,
    pub(crate) gate: Arc<GrpcAuthorizationGate>,
    pub(crate) administration: Arc<AuthorizationPolicyAdministrationService>,
    pub(crate) read_policy: Arc<ReadAuthorizationPolicyUseCase>,
    pub(crate) read_decisions: Arc<ReadAuthorizationDecisionsUseCase>,
    pub(crate) continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
}

pub(crate) async fn fixture_authorization(clock: Arc<dyn ClockPort>) -> FixtureAuthorization {
    let policy_id = AuthorizationPolicyId::new("grpc-fixture").unwrap();
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new("grpc-fixture-host").unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    let store = Arc::new(InMemoryAuthorizationPolicyStore::new());
    let administration = AuthorizationPolicyAdministrationService::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
    );
    administration
        .open(
            principal.clone(),
            vec![SeparationRule::new(
                AuthorizationAction::ApproveCeremonyGuard,
                AuthorizationAction::MountDefinition,
            )
            .unwrap()],
        )
        .await
        .unwrap();
    let (administration_actions, business_actions): (Vec<_>, Vec<_>) = fixture_actions()
        .into_iter()
        .partition(|action| is_administration_action(*action));
    for (grant_id, actions) in [
        ("grpc-fixture-business-actions", business_actions),
        ("grpc-fixture-authorization-admin", administration_actions),
    ] {
        administration
            .issue(
                &principal,
                AuthorizationGrant::new(
                    AuthorizationGrantId::new(grant_id).unwrap(),
                    principal.id().clone(),
                    actions,
                    AuthorizationScope::Global,
                    (clock.now(), None),
                    DelegationDepth::none(),
                    AuthorizationGrantIssuer::direct(principal.clone()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
    }
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60).unwrap(),
    ));
    FixtureAuthorization {
        authorize: authorize.clone(),
        gate: Arc::new(
            GrpcAuthorizationGate::trusted_host(authorize, principal.clone(), "grpc-fixture")
                .expect("fixture authorization must be valid")
                .with_target_digest_proxy_principals(vec![principal]),
        ),
        administration: Arc::new(AuthorizationPolicyAdministrationService::new(
            policy_id.clone(),
            store.clone(),
            clock.clone(),
        )),
        read_policy: Arc::new(ReadAuthorizationPolicyUseCase::new(
            policy_id.clone(),
            store.clone(),
        )),
        read_decisions: Arc::new(ReadAuthorizationDecisionsUseCase::new(
            policy_id.clone(),
            store.clone(),
        )),
        continuation: Arc::new(ContinueAcceptedCeremonyWorkUseCase::new(
            policy_id,
            store,
            clock,
            AuthorizationDecisionTtl::from_seconds(60).unwrap(),
        )),
    }
}

#[allow(clippy::too_many_lines)]
fn fixture_actions() -> Vec<AuthorizationAction> {
    use AuthorizationAction as A;
    vec![
        A::GetCeremonyInstance,
        A::ListCeremonyInstances,
        A::SearchCeremonyInstances,
        A::GenerateCeremonyReport,
        A::ReadCeremonyEvents,
        A::StreamCeremony,
        A::PullCeremonyEvents,
        A::VerifyCeremonyJournal,
        A::GetCeremonyTranscript,
        A::GetCeremonyDefinition,
        A::ListCeremonyDefinitions,
        A::MountDefinition,
        A::ValidateCeremonyDraft,
        A::ExplainCeremonyDraft,
        A::DesignCeremony,
        A::DiffCeremonyDefinitions,
        A::PublishCeremonyDefinition,
        A::RunCeremony,
        A::StartCeremony,
        A::StartPublishedCeremony,
        A::RunCeremonyStep,
        A::ClaimCeremonyStep,
        A::RenewCeremonyStepLease,
        A::ListCeremonyAgents,
        A::GetCeremonyAgent,
        A::ReportCeremonyAgentStatus,
        A::CompleteCeremonyStep,
        A::GetExecutionReceipt,
        A::InspectExecutionRecovery,
        A::CompleteExecutionReceipt,
        A::AdoptExecutionReceipt,
        A::PrepareCeremonyChildren,
        A::AcceptChildCompletion,
        A::RecoverCeremonyChildren,
        A::ApplyCeremonyTransition,
        A::EnforceCeremonyDeadlines,
        A::BindCeremonyParticipants,
        A::PauseCeremony,
        A::ResumeCeremony,
        A::RecordCeremonyHostHandoff,
        A::InspectCeremonyResume,
        A::PlanCeremonySuccessor,
        A::StartCeremonySuccessor,
        A::CancelCeremony,
        A::ApproveCeremonyGuard,
        A::DeferCeremonyGuard,
        A::RequestCeremonyIntervention,
        A::RespondToCeremonyIntervention,
        A::CloseCeremonyIntervention,
        A::PullCeremonyAgentInterventions,
        A::AcknowledgeCeremonyAgentIntervention,
        A::GetCeremonyIntervention,
        A::ListCeremonyInterventions,
        A::BindCeremonyIntegrator,
        A::GetCeremonyIntegratorBinding,
        A::AwaitIntegratorAttention,
        A::AcknowledgeIntegratorAttention,
        A::ListAttentionDeliveries,
        A::CollectCeremonyEvidence,
        A::AssertCeremonyReason,
        A::BeginArtifactUpload,
        A::PutArtifactChunk,
        A::CommitArtifactUpload,
        A::AbortArtifactUpload,
        A::GetArtifact,
        A::ListArtifacts,
        A::ReadArtifactChunk,
        A::TombstoneArtifact,
        A::ReserveBudget,
        A::ReconcileBudget,
        A::ReadBudget,
        A::Deliberate,
        A::StreamDeliberation,
        A::GetDeliberationResult,
        A::Orchestrate,
        A::ProcessTriggerEvent,
        A::RunCouncilDecision,
        A::CreateCouncil,
        A::ListCouncils,
        A::DeleteCouncil,
        A::RegisterAgent,
        A::UnregisterAgent,
        A::RegisterContract,
        A::ListContracts,
        A::DeleteContract,
        A::ReadCouncilEvents,
        A::GetCouncilEventCursor,
        A::LeaseCouncilEvents,
        A::AcknowledgeCouncilEvents,
        A::ReleaseCouncilEvents,
        A::GetStatus,
        A::GetMetrics,
        A::DesignAgenticSystem,
        A::GetAgenticSystem,
        A::ListAgenticSystems,
        A::ValidateAgenticSystem,
        A::PublishAgenticSystem,
        A::InstantiateAgenticSystem,
        A::AdvanceAgenticSystemExecution,
        A::GetAgenticSystemExecution,
        A::RenderAgenticSystemDiagram,
        A::ReadAuthorizationPolicy,
        A::IssueAuthorizationGrant,
        A::RevokeAuthorizationGrant,
        A::ReadAuthorizationDecisions,
    ]
}

fn is_administration_action(action: AuthorizationAction) -> bool {
    matches!(
        action,
        AuthorizationAction::ReadAuthorizationPolicy
            | AuthorizationAction::IssueAuthorizationGrant
            | AuthorizationAction::RevokeAuthorizationGrant
            | AuthorizationAction::ReadAuthorizationDecisions
    )
}
