use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteCeremonyStore;
use made_app::authorization::{
    ContinueAcceptedCeremonyWorkUseCase, ContinueAcceptedStepClaimUseCase,
};
use made_app::services::SessionStream;
use made_core::ports::{AuthorizationPolicyStorePort, NoopCeremonyEventSubscriber};
use made_core::value_objects::{AuthorizationDecisionTtl, AuthorizationPolicyId};

pub(super) fn wire(
    path: &std::path::Path,
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    clock: Arc<SystemClock>,
) -> Result<
    (
        Arc<ContinueAcceptedStepClaimUseCase>,
        Arc<SqliteCeremonyStore>,
    ),
    String,
> {
    let continuation = Arc::new(ContinueAcceptedCeremonyWorkUseCase::new(
        policy_id,
        store,
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60).expect("fixed TTL is valid"),
    ));
    let ceremony_store = Arc::new(
        SqliteCeremonyStore::open(path)
            .map_err(|error| format!("failed to open execution receipt store: {error}"))?,
    );
    let stream = Arc::new(SessionStream::new(
        ceremony_store.clone(),
        ceremony_store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    Ok((
        Arc::new(ContinueAcceptedStepClaimUseCase::new(
            stream,
            continuation,
            clock,
        )),
        ceremony_store,
    ))
}
