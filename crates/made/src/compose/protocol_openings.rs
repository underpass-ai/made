//! Opening a session, and opening the children one plans.
//!
//! One family because they are the four ways a protocol comes into
//! being — from a definition handed in, from a published version, and
//! the two halves of a parent opening and taking back its children —
//! and because all four are built from the same six things.

use std::sync::Arc;

use made_app::services::SessionStream;
use made_app::usecases::{
    AcceptChildCompletionUseCase, PrepareProtocolChildrenUseCase, ResolveProtocolDefinitionUseCase,
    StartProtocolUseCase, StartPublishedProtocolUseCase,
};
use made_core::ports::{
    ClockPort, MemoryReaderPort, ProtocolDefinitionPublicationPort,
    ProtocolDefinitionRepositoryPort,
};

/// The four ways a session, or a child of one, is opened.
pub(super) struct ProtocolOpenings {
    pub(super) start_protocol: Arc<StartProtocolUseCase>,
    pub(super) start_published_protocol: Arc<StartPublishedProtocolUseCase>,
    pub(super) prepare_protocol_children: Arc<PrepareProtocolChildrenUseCase>,
    pub(super) accept_child_completion: Arc<AcceptChildCompletionUseCase>,
}

pub(super) fn wire<C: ClockPort + 'static>(
    resolve_protocol_definition: &Arc<ResolveProtocolDefinitionUseCase>,
    protocol_definitions: &Arc<dyn ProtocolDefinitionRepositoryPort>,
    protocol_publications: &Arc<dyn ProtocolDefinitionPublicationPort>,
    protocol_stream: &Arc<SessionStream>,
    clock: &Arc<C>,
    memory_reader: Arc<dyn MemoryReaderPort>,
) -> ProtocolOpenings {
    // Both run modes use the same stream to carry step results forward.
    let start_protocol = Arc::new(StartProtocolUseCase::new(
        protocol_definitions.clone(),
        protocol_stream.clone(),
        clock.clone(),
        memory_reader.clone(),
    ));
    let start_published_protocol = Arc::new(StartPublishedProtocolUseCase::new(
        protocol_publications.clone(),
        protocol_stream.clone(),
        clock.clone(),
        memory_reader.clone(),
    ));
    let prepare_protocol_children = Arc::new(PrepareProtocolChildrenUseCase::new(
        resolve_protocol_definition.clone(),
        protocol_publications.clone(),
        protocol_stream.clone(),
        clock.clone(),
        memory_reader,
    ));
    let accept_child_completion = Arc::new(AcceptChildCompletionUseCase::new(
        resolve_protocol_definition.clone(),
        protocol_publications.clone(),
        protocol_stream.clone(),
        clock.clone(),
    ));

    ProtocolOpenings {
        start_protocol,
        start_published_protocol,
        prepare_protocol_children,
        accept_child_completion,
    }
}
