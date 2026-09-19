use std::sync::Arc;

use made_adapters::config::{GrpcTlsConfig, ServiceConfig};
use made_adapters::grpc::{GrpcAuthorizationGate, MutualTlsPrincipalMap};
use made_adapters::postgres::{PostgresAuthorizationPolicyStore, PostgresPool};
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationPolicyAdministrationService, AuthorizeOperationUseCase,
    ContinueAcceptedCeremonyWorkUseCase, ReadAuthorizationDecisionsUseCase,
    ReadAuthorizationPolicyUseCase,
};
use made_core::ports::{AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthorizationDecisionTtl, AuthorizationPolicyId, PrincipalId,
};
use made_core::DomainError;

use crate::ComposeError;

const POLICY_ID_ENV: &str = "MADE_AUTH_POLICY_ID";
const PRINCIPALS_PATH_ENV: &str = "MADE_AUTH_MTLS_PRINCIPALS_PATH";
const MCP_PROXY_PRINCIPAL_IDS_ENV: &str = "MADE_AUTH_MCP_PROXY_PRINCIPAL_IDS";

pub(super) struct AuthorizationWiring {
    pub(super) gate: Arc<GrpcAuthorizationGate>,
    authorize: Arc<AuthorizeOperationUseCase>,
    pub(super) administration: Arc<AuthorizationPolicyAdministrationService>,
    pub(super) read_policy: Arc<ReadAuthorizationPolicyUseCase>,
    pub(super) read_decisions: Arc<ReadAuthorizationDecisionsUseCase>,
    pub(super) continuation: Arc<ContinueAcceptedCeremonyWorkUseCase>,
}

impl AuthorizationWiring {
    pub(super) fn protect_runtime(
        &self,
        reader: Arc<dyn made_core::ports::MemoryReaderPort>,
        stream: Arc<made_app::services::SessionStream>,
    ) -> Arc<dyn made_core::ports::MemoryReaderPort> {
        stream.authorize_appends(Arc::new(self.authorize.clone().for_ceremony_appends()));
        Arc::new(made_app::authorization::AuthorizedMemoryReader::new(
            reader,
            self.authorize.clone(),
            stream,
        ))
    }

    pub(super) fn apply(
        self,
        builder: made_adapters::grpc::MadeGrpcServiceBuilder,
    ) -> made_adapters::grpc::MadeGrpcServiceBuilder {
        builder
            .authorization(self.gate)
            .authorization_administration(self.administration)
            .read_authorization_policy(self.read_policy)
            .read_authorization_decisions(self.read_decisions)
    }
}

pub(super) async fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
    clock: Arc<dyn ClockPort>,
) -> Result<AuthorizationWiring, ComposeError> {
    if !matches!(config.grpc_tls, GrpcTlsConfig::Mutual { .. }) {
        return Err(configuration_error(
            "authorization requires MADE_GRPC_TLS_MODE=mutual",
        ));
    }
    let policy_id = AuthorizationPolicyId::new(required_env(POLICY_ID_ENV)?)?;
    let principals = Arc::new(
        MutualTlsPrincipalMap::from_path(required_env(PRINCIPALS_PATH_ENV)?)
            .map_err(|error| configuration_error(error.to_string()))?,
    );
    let proxy_principals = proxy_principals(&principals)?;
    let store = policy_store(config, postgres)?;
    if store.load(&policy_id).await?.is_none() {
        return Err(configuration_error(format!(
            "authorization policy `{}` has not been bootstrapped",
            policy_id.as_str()
        )));
    }
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60)?,
    ));
    let continuation = Arc::new(ContinueAcceptedCeremonyWorkUseCase::new(
        policy_id.clone(),
        store.clone(),
        clock.clone(),
        AuthorizationDecisionTtl::from_seconds(60)?,
    ));
    Ok(AuthorizationWiring {
        gate: Arc::new(
            GrpcAuthorizationGate::mutual_tls(authorize.clone(), principals)
                .with_target_digest_proxy_principals(proxy_principals),
        ),
        authorize,
        administration: Arc::new(AuthorizationPolicyAdministrationService::new(
            policy_id.clone(),
            store.clone(),
            clock,
        )),
        read_policy: Arc::new(ReadAuthorizationPolicyUseCase::new(
            policy_id.clone(),
            store.clone(),
        )),
        read_decisions: Arc::new(ReadAuthorizationDecisionsUseCase::new(policy_id, store)),
        continuation,
    })
}

fn proxy_principals(
    principals: &MutualTlsPrincipalMap,
) -> Result<Vec<AuthenticatedPrincipal>, ComposeError> {
    let Some(raw) = std::env::var(MCP_PROXY_PRINCIPAL_IDS_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(Vec::new());
    };
    raw.split(',')
        .map(|raw_id| {
            if raw_id.trim() != raw_id || raw_id.is_empty() {
                return Err(configuration_error(format!(
                    "{MCP_PROXY_PRINCIPAL_IDS_ENV} must be a comma-separated list without empty or padded ids"
                )));
            }
            let id = PrincipalId::new(raw_id)?;
            let matches = principals
                .principals()
                .filter(|principal| principal.id() == &id)
                .cloned()
                .collect::<Vec<_>>();
            let Some(first) = matches.first() else {
                return Err(configuration_error(format!(
                    "{MCP_PROXY_PRINCIPAL_IDS_ENV} references unmapped principal `{raw_id}`"
                )));
            };
            if matches.iter().any(|candidate| candidate != first) {
                return Err(configuration_error(format!(
                    "{MCP_PROXY_PRINCIPAL_IDS_ENV} principal `{raw_id}` is ambiguous"
                )));
            }
            Ok(first.clone())
        })
        .collect()
}

fn policy_store(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
) -> Result<Arc<dyn AuthorizationPolicyStorePort>, ComposeError> {
    if let Some(pool) = postgres {
        return Ok(Arc::new(PostgresAuthorizationPolicyStore::new(
            pool.clone(),
        )));
    }
    let path = config.ceremony_store_path.as_deref().ok_or_else(|| {
        configuration_error("authorization requires durable Postgres or MADE_CEREMONY_STORE_PATH")
    })?;
    Ok(Arc::new(SqliteAuthorizationPolicyStore::open(path)?))
}

fn required_env(name: &'static str) -> Result<String, ComposeError> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| configuration_error(format!("{name} is required")))
}

fn configuration_error(reason: impl Into<String>) -> ComposeError {
    ComposeError::Domain(DomainError::InvalidDocument {
        reason: reason.into(),
    })
}
