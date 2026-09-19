use super::{
    Arc, CeremonyExecutionConnectorPort, CeremonyWorkerDriver, CeremonyWorkerHost,
    CeremonyWorkerPolicy, CeremonyWorkerStopToken, ClaimCeremonyWorkUseCase,
    CompleteExecutionReceiptUseCase, Config, Duration, EnforceCeremonyDeadlinesUseCase,
    ExecuteCeremonyOperationUseCase, ExecutionConnectorId, ExecutionReceiptStorePort,
    ExecutionRecoveryPageLimit, FileSystemCeremonyDefinitionSource, ForgetfulMemory,
    InMemoryCeremonyDefinitionPublications, InMemoryCeremonyDefinitionRepository,
    InspectExecutionRecoveryUseCase, MaxParallel, MountCeremonyDefinitionsUseCase,
    NoopCeremonyEventSubscriber, Path, RecoverExecutionIntentUseCase, RecoverableCeremonyWorker,
    RepositoryScriptExecutionConnector, ResolveCeremonyDefinitionUseCase, SessionStream,
    SqliteCeremonyStore, StartCeremonyStepUseCase, StartCeremonyUseCase, SystemClock,
};

pub struct Harness {
    pub store: Arc<SqliteCeremonyStore>,
    pub stream: Arc<SessionStream>,
    pub inspector: Arc<InspectExecutionRecoveryUseCase>,
    pub driver: Arc<CeremonyWorkerDriver>,
    pub host: CeremonyWorkerHost,
    pub start: StartCeremonyUseCase,
}

pub async fn compose(
    config: &Config,
    operation_root: &Path,
    definition_root: &Path,
) -> Result<Harness, Box<dyn std::error::Error>> {
    let store = Arc::new(SqliteCeremonyStore::open(&config.store)?);
    let stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopCeremonyEventSubscriber),
    ));
    let definitions = Arc::new(InMemoryCeremonyDefinitionRepository::new());
    MountCeremonyDefinitionsUseCase::new(
        Arc::new(FileSystemCeremonyDefinitionSource::from_directory(
            definition_root,
        )?),
        definitions.clone(),
    )
    .execute()
    .await?;
    let publications = Arc::new(InMemoryCeremonyDefinitionPublications::new());
    let resolver = Arc::new(ResolveCeremonyDefinitionUseCase::new(
        definitions.clone(),
        publications,
    ));
    let clock = Arc::new(SystemClock::new());
    let deadlines = Arc::new(EnforceCeremonyDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = CeremonyWorkerPolicy::new(
        MaxParallel::new(u8::try_from(config.workers)?)?,
        ExecutionRecoveryPageLimit::new(config.workers)?,
    );
    let claims = Arc::new(ClaimCeremonyWorkUseCase::new(
        stream.clone(),
        resolver.clone(),
        deadlines.clone(),
        Arc::new(StartCeremonyStepUseCase::new(
            resolver.clone(),
            stream.clone(),
            clock.clone(),
        )),
        clock.clone(),
        policy,
    ));
    let connector: Arc<dyn CeremonyExecutionConnectorPort> =
        Arc::new(RepositoryScriptExecutionConnector::new(
            ExecutionConnectorId::new("soak.repository-script.v1")?,
            &config.repository,
            &config.script,
            operation_root,
            Duration::from_secs(30),
        )?);
    let receipts: Arc<dyn ExecutionReceiptStorePort> = store.clone();
    let worker = Arc::new(RecoverableCeremonyWorker::new(
        Arc::new(ExecuteCeremonyOperationUseCase::new(
            receipts.clone(),
            connector.clone(),
            clock.clone(),
        )),
        Arc::new(RecoverExecutionIntentUseCase::new(
            receipts.clone(),
            connector,
        )),
        Arc::new(CompleteExecutionReceiptUseCase::new(
            resolver.clone(),
            stream.clone(),
            receipts,
            clock.clone(),
        )),
    ));
    let inspector = Arc::new(InspectExecutionRecoveryUseCase::new(
        stream.clone(),
        store.clone(),
    ));
    let driver = Arc::new(CeremonyWorkerDriver::new(
        inspector.clone(),
        deadlines,
        worker,
        policy,
        CeremonyWorkerStopToken::new(),
    ));
    let host = CeremonyWorkerHost::new(claims, driver.clone());
    let start = StartCeremonyUseCase::new(
        definitions,
        stream.clone(),
        clock,
        Arc::new(ForgetfulMemory::new()),
    );

    Ok(Harness {
        store,
        stream,
        inspector,
        driver,
        host,
        start,
    })
}
