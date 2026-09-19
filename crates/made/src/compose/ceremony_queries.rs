//! Session queries and publication commands share the selected durable stores.
use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::services::SessionStream;
use made_app::usecases::{
    CeremonySearchCursorCodec, CeremonySearchCursorKey, CeremonySearchCursorNamespace,
    DiffCeremonyDefinitionsUseCase, GenerateCeremonyReportUseCase, GetCeremonyInstanceUseCase,
    GetCeremonyTranscriptUseCase, ListCeremonyInstancesUseCase, PublishCeremonyDefinitionUseCase,
    PullCeremonyEventsUseCase, ReadCeremonyEventsUseCase, ResolveCeremonyDefinitionUseCase,
    SearchCeremonyInstancesUseCase, StreamCeremonyUseCase, VerifyCeremonyJournalUseCase,
};
use made_core::ports::{
    CeremonyAgentStatusPort, CeremonyDefinitionPublicationPort, CeremonyEventCursorPort,
    CeremonyEventStorePort, CeremonyInstanceIndexPort, CeremonyProgressNotifierPort,
};
use std::sync::Arc;

use crate::ComposeError;

const CURSOR_KEY_ENV: &str = "MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY";
const STORE_ID_ENV: &str = "MADE_CEREMONY_STORE_ID";
const POLICY_ID_ENV: &str = "MADE_AUTH_POLICY_ID";

pub(super) fn wire(
    builder: MadeGrpcServiceBuilder,
    definition: Arc<ResolveCeremonyDefinitionUseCase>,
    stream: Arc<SessionStream>,
    index: Arc<dyn CeremonyInstanceIndexPort>,
    events: Arc<dyn CeremonyEventStorePort>,
    cursors: Arc<dyn CeremonyEventCursorPort>,
    progress_notifier: Arc<dyn CeremonyProgressNotifierPort>,
    agent_status: Arc<dyn CeremonyAgentStatusPort>,
    publications: Arc<dyn CeremonyDefinitionPublicationPort>,
) -> Result<MadeGrpcServiceBuilder, ComposeError> {
    let search_ceremony_instances = Arc::new(SearchCeremonyInstancesUseCase::new(
        index,
        stream.clone(),
        CeremonySearchCursorCodec::new(
            CeremonySearchCursorKey::from_hex(&required_env(CURSOR_KEY_ENV)?)?,
            CeremonySearchCursorNamespace::new(
                required_env(STORE_ID_ENV)?,
                required_env(POLICY_ID_ENV)?,
            )?,
        ),
    ));
    let get_ceremony_instance = Arc::new(GetCeremonyInstanceUseCase::new(stream.clone()));
    let list_ceremony_instances = Arc::new(ListCeremonyInstancesUseCase::new(stream));
    let read_events = Arc::new(ReadCeremonyEventsUseCase::new(events.clone()));
    let stream_ceremony = Arc::new(
        StreamCeremonyUseCase::new(events.clone(), progress_notifier)
            .with_agent_activity(agent_status),
    );
    let pull_events = Arc::new(PullCeremonyEventsUseCase::new(events.clone(), cursors));
    let verify_ceremony_journal = Arc::new(VerifyCeremonyJournalUseCase::new(events.clone()));
    let get_ceremony_transcript = Arc::new(GetCeremonyTranscriptUseCase::new(events.clone()));
    let generate_ceremony_report = Arc::new(GenerateCeremonyReportUseCase::new(definition, events));
    let publish_ceremony_definition =
        Arc::new(PublishCeremonyDefinitionUseCase::new(publications.clone()));
    let diff_ceremony_definitions = Arc::new(DiffCeremonyDefinitionsUseCase::new(publications));
    Ok(builder
        .get_ceremony_instance(get_ceremony_instance)
        .list_ceremony_instances(list_ceremony_instances)
        .search_ceremony_instances(search_ceremony_instances)
        .read_ceremony_events(read_events)
        .stream_ceremony(stream_ceremony)
        .pull_ceremony_events(pull_events)
        .verify_ceremony_journal(verify_ceremony_journal)
        .get_ceremony_transcript(get_ceremony_transcript)
        .generate_ceremony_report(generate_ceremony_report)
        .publish_ceremony_definition(publish_ceremony_definition)
        .diff_ceremony_definitions(diff_ceremony_definitions))
}

fn required_env(name: &'static str) -> Result<String, ComposeError> {
    std::env::var(name).map_err(|_| {
        ComposeError::CeremonyStore(format!(
            "{name} is required for scoped, restart-stable ceremony search cursors"
        ))
    })
}
