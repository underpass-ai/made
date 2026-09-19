use std::sync::Arc;

use made_app::authorization::ContinueAcceptedCeremonyWorkUseCase;
use made_app::services::SessionStream;
use made_app::usecases::{
    AcceptChildCompletionUseCase, PrepareCeremonyChildrenUseCase, RecoverCeremonyChildrenUseCase,
};
use made_core::ports::{CeremonyEventCursorPort, CeremonyEventStorePort, ClockPort};
use made_core::value_objects::CeremonyEventConsumer;
use made_core::DomainError;

pub(super) struct ChildrenRecoveryDependencies {
    pub(super) events: Arc<dyn CeremonyEventStorePort>,
    pub(super) cursors: Arc<dyn CeremonyEventCursorPort>,
    pub(super) stream: Arc<SessionStream>,
    pub(super) prepare: Arc<PrepareCeremonyChildrenUseCase>,
    pub(super) accept: Arc<AcceptChildCompletionUseCase>,
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
}

pub(super) fn wire(
    dependencies: ChildrenRecoveryDependencies,
) -> Result<RecoverCeremonyChildrenUseCase, DomainError> {
    Ok(RecoverCeremonyChildrenUseCase::new(
        dependencies.events,
        dependencies.cursors,
        dependencies.stream,
        dependencies.prepare,
        dependencies.accept,
        dependencies.clock,
        CeremonyEventConsumer::new("made.children.recovery.v1")?,
    )
    .with_authorization_continuation(dependencies.continuation))
}
