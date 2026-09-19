use std::sync::Arc;

use made_adapters::connectors::{GitExecutionConnector, HttpExecutionConnector};
use made_adapters::execution::{OciExecutionConfig, OciExecutionConnector};
use made_adapters::workers::{FileWorkerAdmissionObserver, FileWorkerCapacityStore};
use made_app::artifacts::ArtifactService;
use made_app::authorization::{
    AuthorizeOperationUseCase, ContinueAcceptedCeremonyWorkUseCase,
    ContinueAcceptedStepClaimUseCase, TrustedHostAuthorizationGate,
};
use made_app::budgets::{BudgetLedgerService, BudgetedStepClaimUseCase};
use made_app::services::SessionStream;
use made_app::usecases::{
    EnforceCeremonyDeadlinesUseCase, ResolveCeremonyDefinitionUseCase, StartCeremonyStepUseCase,
};
use made_app::workers::{
    CeremonyWorkerDriver, CeremonyWorkerHost, CeremonyWorkerRenewal, CeremonyWorkerStopToken,
    ClaimCeremonyWorkInput, ClaimCeremonyWorkUseCase, CompleteExecutionReceiptUseCase,
    ExecuteCeremonyOperationUseCase, InspectExecutionRecoveryUseCase,
    RecoverExecutionIntentUseCase, RecoverableCeremonyWorker, RenewCeremonyStepLeaseUseCase,
};
use made_core::ports::{
    BudgetReservationPlannerPort, CeremonyExecutionConnectorPort, CeremonyInstanceIndexPort,
    ClockPort, ExecutionReceiptStorePort,
};
use made_core::value_objects::{
    AuditActorKind, AuthenticatedPrincipal, AuthenticationMethod, ExecutionConnectorId,
    PrincipalKind,
};
use made_core::DomainError;

use crate::workers::{
    CeremonyWorkerDaemon, ConfiguredWorkerBudgetPlanner, ConfiguredWorkerRootPolicy,
    WorkerAuthorizer, WorkerConnectorKind, WorkerDaemonConfig,
};
use crate::ComposeError;

pub(super) struct CeremonyWorkerDependencies {
    pub(super) index: Arc<dyn CeremonyInstanceIndexPort>,
    pub(super) stream: Arc<SessionStream>,
    pub(super) definitions: Arc<ResolveCeremonyDefinitionUseCase>,
    pub(super) deadlines: Arc<EnforceCeremonyDeadlinesUseCase>,
    pub(super) start_step: Arc<StartCeremonyStepUseCase>,
    pub(super) receipts: Arc<dyn ExecutionReceiptStorePort>,
    pub(super) clock: Arc<dyn ClockPort>,
    pub(super) artifacts: Option<Arc<ArtifactService>>,
    pub(super) budgets: BudgetLedgerService,
    pub(super) budgeted_claim: Arc<BudgetedStepClaimUseCase>,
    pub(super) budget_planner: Option<Arc<dyn BudgetReservationPlannerPort>>,
    pub(super) authorize: Arc<AuthorizeOperationUseCase>,
    pub(super) continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
}

pub(super) fn wire(
    dependencies: CeremonyWorkerDependencies,
) -> Result<Option<Arc<CeremonyWorkerDaemon>>, ComposeError> {
    let Some(config) = WorkerDaemonConfig::from_env()? else {
        return Ok(None);
    };
    let connector_id = ExecutionConnectorId::new(config.connector.id())?;
    let connector = connector(&config, connector_id.clone())?;
    let worker = recoverable_worker(&dependencies, connector);
    let capacity = Arc::new(FileWorkerCapacityStore::new(
        &config.capacity_directory,
        config.capacity_limits,
        dependencies.stream.clone(),
    )?);
    let principal = AuthenticatedPrincipal::new(
        config.principal.clone(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )?;
    let gate = TrustedHostAuthorizationGate::new(dependencies.authorize.clone(), principal)?;
    let continuation = Arc::new(
        ContinueAcceptedStepClaimUseCase::new(
            dependencies.stream.clone(),
            dependencies.continuation,
            dependencies.clock.clone(),
        )
        .with_reauthorization(dependencies.authorize),
    );
    let authorization = Arc::new(WorkerAuthorizer::new(gate, continuation));
    let root_policy = Arc::new(ConfiguredWorkerRootPolicy::from_json(
        config.admission_policy.root_policy(),
        config.root_policies_json.as_deref(),
    )?);
    let admission_observer = Arc::new(FileWorkerAdmissionObserver::new(&config.admission_log)?);

    let mut claims = ClaimCeremonyWorkUseCase::new(
        dependencies.index,
        dependencies.stream.clone(),
        dependencies.definitions.clone(),
        dependencies.deadlines.clone(),
        dependencies.start_step,
        dependencies.clock.clone(),
        config.worker_policy,
    )
    .with_shared_capacity(capacity.clone(), connector_id)
    .with_authorization(authorization.clone())
    .with_admission_policy(config.admission_policy)
    .with_admission_observer(admission_observer)
    .with_root_policy(root_policy);
    let planner: Arc<dyn BudgetReservationPlannerPort> =
        if let Some(planner) = dependencies.budget_planner {
            planner
        } else {
            Arc::new(ConfiguredWorkerBudgetPlanner::from_env()?)
        };
    claims = claims.with_budget_admission(dependencies.budgeted_claim, planner);

    let renewal = CeremonyWorkerRenewal::new(
        Arc::new(RenewCeremonyStepLeaseUseCase::new(
            dependencies.stream.clone(),
            dependencies.definitions,
            dependencies.clock,
        )),
        config.owner.clone(),
        config.lease_ttl,
        config.heartbeat,
    )?
    .with_shared_capacity(capacity)
    .with_authorization(authorization.clone());
    let stop = CeremonyWorkerStopToken::new();
    let driver = Arc::new(
        CeremonyWorkerDriver::new(
            Arc::new(InspectExecutionRecoveryUseCase::new(
                dependencies.stream,
                dependencies.receipts,
            )),
            dependencies.deadlines,
            worker,
            config.worker_policy,
            stop.clone(),
        )
        .with_renewal(Arc::new(renewal))
        .with_authorization(authorization),
    );
    let host = Arc::new(CeremonyWorkerHost::new(Arc::new(claims), driver));
    let input = ClaimCeremonyWorkInput::new(
        None,
        config.claim_page,
        config.owner,
        config.lease_ttl,
        AuditActorKind::Service,
    );
    Ok(Some(Arc::new(CeremonyWorkerDaemon::new(
        host,
        input,
        config.host_policy,
        stop,
    ))))
}

fn recoverable_worker(
    dependencies: &CeremonyWorkerDependencies,
    connector: Arc<dyn CeremonyExecutionConnectorPort>,
) -> Arc<RecoverableCeremonyWorker> {
    let mut execute = ExecuteCeremonyOperationUseCase::new(
        dependencies.receipts.clone(),
        connector.clone(),
        dependencies.clock.clone(),
    );
    let mut recover = RecoverExecutionIntentUseCase::new(dependencies.receipts.clone(), connector);
    let mut complete = CompleteExecutionReceiptUseCase::new(
        dependencies.definitions.clone(),
        dependencies.stream.clone(),
        dependencies.receipts.clone(),
        dependencies.clock.clone(),
    )
    .with_budget_ledger(dependencies.budgets.clone());
    if let Some(artifacts) = &dependencies.artifacts {
        execute = execute.with_artifacts(artifacts.clone());
        recover = recover.with_artifacts(artifacts.clone());
        complete = complete.with_artifacts(artifacts.clone());
    }
    Arc::new(RecoverableCeremonyWorker::new(
        Arc::new(execute),
        Arc::new(recover),
        Arc::new(complete),
    ))
}

fn connector(
    config: &WorkerDaemonConfig,
    id: ExecutionConnectorId,
) -> Result<Arc<dyn CeremonyExecutionConnectorPort>, DomainError> {
    match config.connector {
        WorkerConnectorKind::Oci => Ok(Arc::new(OciExecutionConnector::new(
            id,
            OciExecutionConfig {
                image: required(config.oci_image.as_ref(), "MADE_WORKER_OCI_IMAGE")?,
                workspace: required(config.oci_workspace.as_ref(), "MADE_WORKER_OCI_WORKSPACE")?,
                operation_root: config.operation_root.clone(),
                network: config.oci_network.clone(),
                uid: config.oci_uid,
                cpus: config.oci_cpus,
                memory_bytes: config.oci_memory_bytes,
                pids: config.oci_pids,
                max_output_bytes: config.oci_max_output_bytes,
                timeout: config.connector_timeout,
            },
        )?)),
        WorkerConnectorKind::Git => Ok(Arc::new(GitExecutionConnector::new(
            id,
            required(config.git_repository.as_ref(), "MADE_WORKER_GIT_REPOSITORY")?,
            required(config.git_scratch.as_ref(), "MADE_WORKER_GIT_SCRATCH")?,
        )?)),
        WorkerConnectorKind::Http => Ok(Arc::new(HttpExecutionConnector::new(
            id,
            &required(config.http_base.as_ref(), "MADE_WORKER_HTTP_BASE")?,
            &config.operation_root,
            config.connector_timeout,
        )?)),
    }
}

fn required<T: Clone>(value: Option<&T>, name: &'static str) -> Result<T, DomainError> {
    value.cloned().ok_or_else(|| DomainError::InvalidDocument {
        reason: format!("{name} is required for the selected worker connector"),
    })
}
