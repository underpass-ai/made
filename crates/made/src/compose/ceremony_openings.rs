//! Opening a session, and opening the children one plans.
//!
//! One family because they are the four ways a ceremony comes into
//! being — from a definition handed in, from a published version, and
//! the two halves of a parent opening and taking back its children —
//! and because all four are built from the same six things.

use std::sync::Arc;

use made_app::services::SessionStream;
use made_app::usecases::{
    AcceptChildCompletionUseCase, PrepareCeremonyChildrenUseCase, ResolveCeremonyDefinitionUseCase,
    StartCeremonyUseCase, StartPublishedCeremonyUseCase,
};
use made_core::ports::{
    CeremonyDefinitionPublicationPort, CeremonyDefinitionRepositoryPort, ClockPort,
    MemoryReaderPort,
};

/// The four ways a session, or a child of one, is opened.
pub(super) struct CeremonyOpenings {
    pub(super) start_ceremony: Arc<StartCeremonyUseCase>,
    pub(super) start_published_ceremony: Arc<StartPublishedCeremonyUseCase>,
    pub(super) prepare_ceremony_children: Arc<PrepareCeremonyChildrenUseCase>,
    pub(super) accept_child_completion: Arc<AcceptChildCompletionUseCase>,
}

pub(super) fn wire<C: ClockPort + 'static>(
    resolve_ceremony_definition: &Arc<ResolveCeremonyDefinitionUseCase>,
    ceremony_definitions: &Arc<dyn CeremonyDefinitionRepositoryPort>,
    ceremony_publications: &Arc<dyn CeremonyDefinitionPublicationPort>,
    ceremony_stream: &Arc<SessionStream>,
    clock: &Arc<C>,
    memory_reader: Arc<dyn MemoryReaderPort>,
) -> CeremonyOpenings {
    // Both run modes use the same stream to carry step results forward.
    let start_ceremony = Arc::new(StartCeremonyUseCase::new(
        ceremony_definitions.clone(),
        ceremony_stream.clone(),
        clock.clone(),
        memory_reader.clone(),
    ));
    let start_published_ceremony = Arc::new(StartPublishedCeremonyUseCase::new(
        ceremony_publications.clone(),
        ceremony_stream.clone(),
        clock.clone(),
        memory_reader.clone(),
    ));
    let prepare_ceremony_children = Arc::new(PrepareCeremonyChildrenUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_publications.clone(),
        ceremony_stream.clone(),
        clock.clone(),
        memory_reader,
    ));
    let accept_child_completion = Arc::new(AcceptChildCompletionUseCase::new(
        resolve_ceremony_definition.clone(),
        ceremony_publications.clone(),
        ceremony_stream.clone(),
        clock.clone(),
    ));

    CeremonyOpenings {
        start_ceremony,
        start_published_ceremony,
        prepare_ceremony_children,
        accept_child_completion,
    }
}
