use std::sync::Arc;

use made_adapters::grpc::GrpcAuthorizationGate;
use made_adapters::memory::InMemoryAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
};
use made_core::ports::ClockPort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationDecisionTtl,
    AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId,
    AuthorizationScope, DelegationDepth, PrincipalId, PrincipalKind,
};

pub(crate) async fn fixture_authorization(clock: Arc<dyn ClockPort>) -> Arc<GrpcAuthorizationGate> {
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
        .open(principal.clone(), Vec::new())
        .await
        .unwrap();
    administration
        .issue(
            &principal,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("grpc-fixture-all-actions").unwrap(),
                principal.id().clone(),
                fixture_actions(),
                AuthorizationScope::Global,
                (clock.now(), None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(principal.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id,
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(300).unwrap(),
    ));
    Arc::new(
        GrpcAuthorizationGate::trusted_host(authorize, principal, "grpc-fixture")
            .expect("fixture authorization must be valid"),
    )
}

#[allow(clippy::too_many_lines)]
fn fixture_actions() -> Vec<AuthorizationAction> {
    use AuthorizationAction as A;
    vec![
        A::GetCeremonyInstance,
        A::ListCeremonyInstances,
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
        A::CompleteCeremonyStep,
        A::PrepareCeremonyChildren,
        A::AcceptChildCompletion,
        A::RecoverCeremonyChildren,
        A::ApplyCeremonyTransition,
        A::EnforceCeremonyDeadlines,
        A::BindCeremonyParticipants,
        A::PauseCeremony,
        A::ResumeCeremony,
        A::CancelCeremony,
        A::ApproveCeremonyGuard,
        A::DeferCeremonyGuard,
        A::RequestCeremonyIntervention,
        A::RespondToCeremonyIntervention,
        A::CloseCeremonyIntervention,
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
        A::ReadAuthorizationPolicy,
        A::IssueAuthorizationGrant,
        A::RevokeAuthorizationGrant,
        A::ReadAuthorizationDecisions,
    ]
}
