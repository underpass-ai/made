use super::{
    Arc, ClaimProtocolWorkUseCase, CompleteExecutionReceiptUseCase, Config, Duration,
    EnforceProtocolDeadlinesUseCase, ExecuteProtocolOperationUseCase, ExecutionConnectorId,
    ExecutionReceiptStorePort, ExecutionRecoveryPageLimit, FileSystemProtocolDefinitionSource,
    ForgetfulMemory, InMemoryProtocolDefinitionPublications, InMemoryProtocolDefinitionRepository,
    InspectExecutionRecoveryUseCase, MaxParallel, MountProtocolDefinitionsUseCase,
    NoopProtocolEventSubscriber, Path, ProtocolExecutionConnectorPort, ProtocolWorkerDriver,
    ProtocolWorkerHost, ProtocolWorkerPolicy, ProtocolWorkerStopToken,
    RecoverExecutionIntentUseCase, RecoverableProtocolWorker, RepositoryScriptExecutionConnector,
    ResolveProtocolDefinitionUseCase, SessionStream, SqliteProtocolStore, StartProtocolStepUseCase,
    StartProtocolUseCase, SystemClock,
};

pub struct Harness {
    pub store: Arc<SqliteProtocolStore>,
    pub stream: Arc<SessionStream>,
    pub inspector: Arc<InspectExecutionRecoveryUseCase>,
    pub driver: Arc<ProtocolWorkerDriver>,
    pub host: ProtocolWorkerHost,
    pub start: StartProtocolUseCase,
}

pub async fn compose(
    config: &Config,
    operation_root: &Path,
    definition_root: &Path,
) -> Result<Harness, Box<dyn std::error::Error>> {
    let store = Arc::new(SqliteProtocolStore::open(&config.store)?);
    let stream = Arc::new(SessionStream::new(
        store.clone(),
        store.clone(),
        Arc::new(NoopProtocolEventSubscriber),
    ));
    let definitions = Arc::new(InMemoryProtocolDefinitionRepository::new());
    MountProtocolDefinitionsUseCase::new(
        Arc::new(FileSystemProtocolDefinitionSource::from_directory(
            definition_root,
        )?),
        definitions.clone(),
    )
    .execute()
    .await?;
    let publications = Arc::new(InMemoryProtocolDefinitionPublications::new());
    let resolver = Arc::new(ResolveProtocolDefinitionUseCase::new(
        definitions.clone(),
        publications,
    ));
    let clock = Arc::new(SystemClock::new());
    let deadlines = Arc::new(EnforceProtocolDeadlinesUseCase::new(
        resolver.clone(),
        stream.clone(),
        clock.clone(),
    ));
    let policy = ProtocolWorkerPolicy::new(
        MaxParallel::new(u8::try_from(config.workers)?)?,
        ExecutionRecoveryPageLimit::new(config.workers)?,
    );
    let claims = Arc::new(ClaimProtocolWorkUseCase::new(
        store.clone(),
        stream.clone(),
        resolver.clone(),
        deadlines.clone(),
        Arc::new(StartProtocolStepUseCase::new(
            resolver.clone(),
            stream.clone(),
            clock.clone(),
        )),
        clock.clone(),
        policy,
    ));
    let connector: Arc<dyn ProtocolExecutionConnectorPort> =
        Arc::new(RepositoryScriptExecutionConnector::new(
            ExecutionConnectorId::new("soak.repository-script.v1")?,
            &config.repository,
            &config.script,
            operation_root,
            Duration::from_secs(30),
        )?);
    let receipts: Arc<dyn ExecutionReceiptStorePort> = store.clone();
    let worker = Arc::new(RecoverableProtocolWorker::new(
        Arc::new(ExecuteProtocolOperationUseCase::new(
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
    let driver = Arc::new(ProtocolWorkerDriver::new(
        inspector.clone(),
        deadlines,
        worker,
        policy,
        ProtocolWorkerStopToken::new(),
    ));
    let host = ProtocolWorkerHost::new(claims, driver.clone());
    let start = StartProtocolUseCase::new(
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
