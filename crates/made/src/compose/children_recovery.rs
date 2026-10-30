use std::sync::Arc;

use made_app::authorization::ContinueAcceptedProtocolWorkUseCase;
use made_app::services::SessionStream;
use made_app::usecases::{
    AcceptChildCompletionUseCase, PrepareProtocolChildrenUseCase, RecoverProtocolChildrenUseCase,
};
use made_core::ports::{ClockPort, ProtocolEventCursorPort, ProtocolEventStorePort};
use made_core::value_objects::ProtocolEventConsumer;
use made_core::DomainError;

pub(super) struct ChildrenRecoveryDependencies {
    pub(super) events: Arc<dyn ProtocolEventStorePort>,
    pub(super) cursors: Arc<dyn ProtocolEventCursorPort>,
    pub(super) stream: Arc<SessionStream>,
    pub(super) prepare: Arc<PrepareProtocolChildrenUseCase>,
    pub(super) accept: Arc<AcceptChildCompletionUseCase>,
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) continuation: Arc<ContinueAcceptedProtocolWorkUseCase>,
}

pub(super) fn wire(
    dependencies: ChildrenRecoveryDependencies,
) -> Result<RecoverProtocolChildrenUseCase, DomainError> {
    Ok(RecoverProtocolChildrenUseCase::new(
        dependencies.events,
        dependencies.cursors,
        dependencies.stream,
        dependencies.prepare,
        dependencies.accept,
        dependencies.clock,
        ProtocolEventConsumer::new("made.children.recovery.v1")?,
    )
    .with_authorization_continuation(dependencies.continuation))
}
