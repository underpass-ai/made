//! Session queries and publication commands share the selected durable stores.
use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::services::SessionStream;
use made_app::usecases::{
    DiffCeremonyDefinitionsUseCase, GenerateCeremonyReportUseCase, GetCeremonyInstanceUseCase,
    GetCeremonyTranscriptUseCase, ListCeremonyInstancesUseCase, PublishCeremonyDefinitionUseCase,
    PullCeremonyEventsUseCase, ReadCeremonyEventsUseCase, ResolveCeremonyDefinitionUseCase,
    StreamCeremonyUseCase, VerifyCeremonyJournalUseCase,
};
use made_core::ports::{
    CeremonyDefinitionPublicationPort, CeremonyEventCursorPort, CeremonyEventStorePort,
    CeremonyProgressNotifierPort,
};
use std::sync::Arc;

pub(super) fn wire(
    builder: MadeGrpcServiceBuilder,
    definition: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    progress_notifier: Arc<dyn CeremonyProgressNotifierPort>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
) -> MadeGrpcServiceBuilder {
    let get_ceremony_instance = Arc::new(GetCeremonyInstanceUseCase::new(stream.clone()));
    let list_ceremony_instances = Arc::new(ListCeremonyInstancesUseCase::new(stream));
    let read_events = Arc::new(ReadCeremonyEventsUseCase::new(events.clone()));
    let stream_ceremony = Arc::new(StreamCeremonyUseCase::new(
        events.clone(),
        progress_notifier,
    ));
    let pull_events = Arc::new(PullCeremonyEventsUseCase::new(events.clone(), cursors));
    let verify_ceremony_journal = Arc::new(VerifyCeremonyJournalUseCase::new(events.clone()));
    let get_ceremony_transcript = Arc::new(GetCeremonyTranscriptUseCase::new(events.clone()));
    let generate_ceremony_report = Arc::new(GenerateCeremonyReportUseCase::new(definition, events));
    let publish_ceremony_definition =
        Arc::new(PublishCeremonyDefinitionUseCase::new(publications.clone()));
    let diff_ceremony_definitions = Arc::new(DiffCeremonyDefinitionsUseCase::new(publications));
    builder
        .get_ceremony_instance(get_ceremony_instance)
        .list_ceremony_instances(list_ceremony_instances)
        .read_ceremony_events(read_events)
        .stream_ceremony(stream_ceremony)
        .pull_ceremony_events(pull_events)
        .verify_ceremony_journal(verify_ceremony_journal)
        .get_ceremony_transcript(get_ceremony_transcript)
        .generate_ceremony_report(generate_ceremony_report)
        .publish_ceremony_definition(publish_ceremony_definition)
        .diff_ceremony_definitions(diff_ceremony_definitions)
}
