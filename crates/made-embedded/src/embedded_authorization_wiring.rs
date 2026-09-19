use std::sync::Arc;

use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase, AuthorizedMemoryReader,
    ContinueAcceptedCeremonyWorkUseCase, ReadAuthorizationDecisionsUseCase,
    ReadAuthorizationPolicyUseCase,
};
use made_app::services::SessionStream;
use made_core::ports::{AuthorizationPolicyStorePort, ClockPort, MemoryReaderPort};
use made_core::value_objects::{AuthorizationDecisionTtl, AuthorizationPolicyId};

use crate::embedded_authorization_services::EmbeddedAuthorizationServices;

pub(crate) fn wire(
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    clock: Arc<dyn ClockPort>,
    memory_reader: Arc<dyn MemoryReaderPort>,
    stream: &Arc<SessionStream>,
) -> (Arc<dyn MemoryReaderPort>, EmbeddedAuthorizationServices) {
    let ttl = AuthorizationDecisionTtl::from_seconds(60)
        .expect("fixed embedded authorization TTL is valid");
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
        ttl,
    ));
    let memory_reader = Arc::new(AuthorizedMemoryReader::new(
        memory_reader,
        authorize.clone(),
        Arc::clone(stream),
    ));
    let continuation = ContinueAcceptedCeremonyWorkUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
        ttl,
    );
    stream.authorize_appends(Arc::new(
        made_app::authorization::AuthorizeCeremonyAppendUseCase::new(
            authorize.clone(),
            Arc::new(continuation.clone()),
            clock.clone(),
        ),
    ));
    let services = EmbeddedAuthorizationServices::new(
        ReadAuthorizationPolicyUseCase::new(policy_id.clone(), store.clone()),
        ReadAuthorizationDecisionsUseCase::new(policy_id.clone(), store.clone()),
        AuthorizationPolicyAdministrationService::new(policy_id, store, clock),
        continuation,
        authorize,
    );
    (memory_reader, services)
}
