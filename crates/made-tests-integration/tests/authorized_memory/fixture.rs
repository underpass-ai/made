use std::sync::Arc;

use made_adapters::memory::{
    InMemoryAuthorizationPolicyStore, InMemoryCeremonyEventStore, InProcessSessionMemory,
};
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase, AuthorizedMemoryReader,
    TrustedHostAuthorizationGate,
};
use made_app::services::SessionStream;
use made_core::ports::{MemoryWriterPort, NoopCeremonyEventSubscriber};
use made_core::value_objects::{
    Attributes, AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction,
    AuthorizationDecisionTtl, AuthorizationGrant, AuthorizationGrantId, AuthorizationGrantIssuer,
    AuthorizationPolicyId, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    AuthorizedOperation, CeremonyId, DelegationDepth, MemoryConfidence, MemoryEntry, MemoryEntryId,
    MemoryEntryKind, MemoryProvenance, MemoryRelation, MemoryRelationKind, MemoryScope,
    MemoryWrite, PrincipalId, PrincipalKind,
};
use made_tests_integration::parity_clock::{ParityClock, PARITY_INSTANT};
use time::{Duration, OffsetDateTime};

pub struct Fixture {
    pub memory: Arc<InProcessSessionMemory>,
    pub reader: AuthorizedMemoryReader,
    pub admin: AuthorizationPolicyAdministrationService,
    pub policies: Arc<InMemoryAuthorizationPolicyStore>,
    pub owner: AuthenticatedPrincipal,
    pub policy_id: AuthorizationPolicyId,
    pub scope: MemoryScope,
    gate: TrustedHostAuthorizationGate,
    grantee: PrincipalId,
}

impl Fixture {
    pub async fn new() -> Self {
        let policies = Arc::new(InMemoryAuthorizationPolicyStore::new());
        let policy_id = AuthorizationPolicyId::new("memory-policy").unwrap();
        let clock = ParityClock::shared();
        let owner = principal("owner");
        let caller = principal("caller");
        let admin = AuthorizationPolicyAdministrationService::new(
            policy_id.clone(),
            policies.clone(),
            clock.clone(),
        );
        admin.open(owner.clone(), vec![]).await.unwrap();
        let authorize = Arc::new(AuthorizeOperationUseCase::new(
            policy_id.clone(),
            policies.clone(),
            clock,
            AuthorizationDecisionTtl::from_seconds(60).unwrap(),
        ));
        let events = Arc::new(InMemoryCeremonyEventStore::new());
        let sources = Arc::new(SessionStream::new_authorized(
            events.clone(),
            events,
            Arc::new(NoopCeremonyEventSubscriber),
        ));
        let memory = Arc::new(InProcessSessionMemory::new());
        let reader = AuthorizedMemoryReader::new(memory.clone(), authorize.clone(), sources);
        let gate = TrustedHostAuthorizationGate::new(authorize, caller.clone()).unwrap();
        let fixture = Self {
            memory,
            reader,
            admin,
            policies,
            owner,
            policy_id,
            scope: MemoryScope::new("team:shared-workspace").unwrap(),
            gate,
            grantee: caller.id().clone(),
        };
        fixture
            .issue(
                "start",
                AuthorizationAction::StartPublishedCeremony,
                AuthorizationScope::Global,
                None,
            )
            .await;
        let entries = vec![
            entry("a1", "source-a"),
            entry("a2", "source-a"),
            entry("b1", "source-b"),
        ];
        let relations = vec![
            relation("a1", "a2", "visible explanation"),
            relation("a2", "b1", "private source-b explanation"),
        ];
        fixture
            .memory
            .remember(
                &fixture.scope,
                MemoryWrite::new(entries, relations).unwrap(),
                "initial-memory",
            )
            .await
            .unwrap();
        fixture
    }

    pub async fn issue(
        &self,
        id: &str,
        action: AuthorizationAction,
        scope: AuthorizationScope,
        valid_until: Option<OffsetDateTime>,
    ) {
        let grant = AuthorizationGrant::new(
            AuthorizationGrantId::new(id).unwrap(),
            self.grantee.clone(),
            [action],
            scope,
            (PARITY_INSTANT - Duration::hours(1), valid_until),
            DelegationDepth::none(),
            AuthorizationGrantIssuer::direct(self.owner.clone()),
        )
        .unwrap();
        self.admin.issue(&self.owner, grant).await.unwrap();
    }

    pub async fn read_grant(&self, id: &str, source: &str) {
        self.issue(
            id,
            AuthorizationAction::ReadCeremonyEvents,
            AuthorizationScope::Ceremony {
                ceremony_id: CeremonyId::new(source).unwrap(),
            },
            None,
        )
        .await;
    }

    pub async fn operation(&self, id: &str) -> AuthorizedOperation {
        self.gate
            .authorize(
                AuthorizationRequestId::new(id).unwrap(),
                AuthorizationAction::StartPublishedCeremony,
                AuthorizationScope::Global,
                AuthorizationTargetDigest::for_bytes(id.as_bytes()),
                None,
            )
            .await
            .unwrap()
    }
}

fn principal(id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal::new(
        PrincipalId::new(id).unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap()
}

pub fn entry(id: &str, source: &str) -> MemoryEntry {
    MemoryEntry::new(
        MemoryEntryId::new(id).unwrap(),
        MemoryEntryKind::Decision,
        format!("private decision from {source}"),
        MemoryProvenance::new(CeremonyId::new(source).unwrap(), None, PARITY_INSTANT),
        Attributes::empty(),
    )
    .unwrap()
}

pub fn relation(from: &str, to: &str, why: &str) -> MemoryRelation {
    MemoryRelation::new(
        MemoryEntryId::new(from).unwrap(),
        MemoryEntryId::new(to).unwrap(),
        MemoryRelationKind::ChosenBecause,
        why,
        MemoryConfidence::High,
    )
    .unwrap()
}
