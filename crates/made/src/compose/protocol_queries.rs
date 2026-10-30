//! Session queries and publication commands share the selected durable stores.
use made_adapters::grpc::MadeGrpcServiceBuilder;
use made_app::services::SessionStream;
use made_app::usecases::{
    DiffProtocolDefinitionsUseCase, GenerateProtocolReportUseCase, GetProtocolInstanceUseCase,
    GetProtocolTranscriptUseCase, ListProtocolInstancesUseCase, ProtocolSearchCursorCodec,
    ProtocolSearchCursorKey, ProtocolSearchCursorNamespace, PublishProtocolDefinitionUseCase,
    PullProtocolEventsUseCase, ReadProtocolEventsUseCase, ResolveProtocolDefinitionUseCase,
    SearchProtocolInstancesUseCase, StreamProtocolUseCase, VerifyProtocolJournalUseCase,
};
use made_core::ports::{
    ProtocolAgentStatusPort, ProtocolDefinitionPublicationPort, ProtocolEventCursorPort,
    ProtocolEventStorePort, ProtocolInstanceIndexPort, ProtocolProgressNotifierPort,
};
use std::sync::Arc;

use crate::ComposeError;

const CURSOR_KEY_ENV: &str = "MADE_PROTOCOL_SEARCH_CURSOR_HMAC_KEY";
const STORE_ID_ENV: &str = "MADE_PROTOCOL_STORE_ID";
const POLICY_ID_ENV: &str = "MADE_AUTH_POLICY_ID";

pub(super) fn wire(
    builder: MadeGrpcServiceBuilder,
    definition: Arc<ResolveProtocolDefinitionUseCase>,
    stream: Arc<SessionStream>,
    index: Arc<dyn ProtocolInstanceIndexPort>,
    events: Arc<dyn ProtocolEventStorePort>,
    cursors: Arc<dyn ProtocolEventCursorPort>,
    progress_notifier: Arc<dyn ProtocolProgressNotifierPort>,
    agent_status: Arc<dyn ProtocolAgentStatusPort>,
    publications: Arc<dyn ProtocolDefinitionPublicationPort>,
) -> Result<MadeGrpcServiceBuilder, ComposeError> {
    let search_protocol_instances = Arc::new(SearchProtocolInstancesUseCase::new(
        index,
        stream.clone(),
        ProtocolSearchCursorCodec::new(
            ProtocolSearchCursorKey::from_hex(&required_env(CURSOR_KEY_ENV)?)?,
            ProtocolSearchCursorNamespace::new(
                required_env(STORE_ID_ENV)?,
                required_env(POLICY_ID_ENV)?,
            )?,
        ),
    ));
    let get_protocol_instance = Arc::new(GetProtocolInstanceUseCase::new(stream.clone()));
    let list_protocol_instances = Arc::new(ListProtocolInstancesUseCase::new(stream));
    let read_events = Arc::new(ReadProtocolEventsUseCase::new(events.clone()));
    let stream_protocol = Arc::new(
        StreamProtocolUseCase::new(events.clone(), progress_notifier)
            .with_agent_activity(agent_status),
    );
    let pull_events = Arc::new(PullProtocolEventsUseCase::new(events.clone(), cursors));
    let verify_protocol_journal = Arc::new(VerifyProtocolJournalUseCase::new(events.clone()));
    let get_protocol_transcript = Arc::new(GetProtocolTranscriptUseCase::new(events.clone()));
    let generate_protocol_report = Arc::new(GenerateProtocolReportUseCase::new(definition, events));
    let publish_protocol_definition =
        Arc::new(PublishProtocolDefinitionUseCase::new(publications.clone()));
    let diff_protocol_definitions =
        Arc::new(DiffProtocolDefinitionsUseCase::new(publications.clone()));
    Ok(builder
        .get_protocol_instance(get_protocol_instance)
        .list_protocol_instances(list_protocol_instances)
        .search_protocol_instances(search_protocol_instances)
        .read_protocol_events(read_events)
        .stream_protocol(stream_protocol)
        .pull_protocol_events(pull_events)
        .verify_protocol_journal(verify_protocol_journal)
        .get_protocol_transcript(get_protocol_transcript)
        .generate_protocol_report(generate_protocol_report)
        .publish_protocol_definition(publish_protocol_definition)
        .protocol_publications(publications)
        .diff_protocol_definitions(diff_protocol_definitions))
}

fn required_env(name: &'static str) -> Result<String, ComposeError> {
    std::env::var(name).map_err(|_| {
        ComposeError::ProtocolStore(format!(
            "{name} is required for scoped, restart-stable protocol search cursors"
        ))
    })
}
