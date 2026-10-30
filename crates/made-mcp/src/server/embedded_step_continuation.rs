use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteProtocolStore;
use made_app::authorization::{
    ContinueAcceptedProtocolWorkUseCase, ContinueAcceptedStepClaimUseCase,
};
use made_app::services::SessionStream;
use made_core::ports::{AuthorizationPolicyStorePort, NoopProtocolEventSubscriber};
use made_core::value_objects::{AuthorizationDecisionTtl, AuthorizationPolicyId};

pub(super) fn wire(
    path: &std::path::Path,
    policy_id: AuthorizationPolicyId,
    store: Arc<dyn AuthorizationPolicyStorePort>,
    clock: Arc<SystemClock>,
) -> Result<
    (
        Arc<ContinueAcceptedStepClaimUseCase>,
        Arc<SqliteProtocolStore>,
    ),
    String,
> {
    let continuation = Arc::new(ContinueAcceptedProtocolWorkUseCase::new(
        policy_id,
        store,
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60).expect("fixed TTL is valid"),
    ));
    let protocol_store = Arc::new(
        SqliteProtocolStore::open(path)
            .map_err(|error| format!("failed to open execution receipt store: {error}"))?,
    );
    let stream = Arc::new(SessionStream::new(
        protocol_store.clone(),
        protocol_store.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    Ok((
        Arc::new(ContinueAcceptedStepClaimUseCase::new(
            stream,
            continuation,
            clock,
        )),
        protocol_store,
    ))
}
