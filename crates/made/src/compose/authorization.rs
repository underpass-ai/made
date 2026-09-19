use std::sync::Arc;

use made_adapters::config::{GrpcTlsConfig, ServiceConfig};
use made_adapters::grpc::{GrpcAuthorizationGate, MutualTlsPrincipalMap};
use made_adapters::postgres::{PostgresAuthorizationPolicyStore, PostgresPool};
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::AuthorizeOperationUseCase;
use made_core::ports::{AuthorizationPolicyStorePort, ClockPort};
use made_core::value_objects::{AuthorizationDecisionTtl, AuthorizationPolicyId};
use made_core::DomainError;

use crate::ComposeError;

const POLICY_ID_ENV: &str = "MADE_AUTH_POLICY_ID";
const PRINCIPALS_PATH_ENV: &str = "MADE_AUTH_MTLS_PRINCIPALS_PATH";

pub(super) async fn wire(
    config: &ServiceConfig,
    postgres: Option<&PostgresPool>,
    clock: Arc<dyn ClockPort>,
) -> Result<Arc<GrpcAuthorizationGate>, ComposeError> {
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
    let store = policy_store(config, postgres)?;
    if store.load(&policy_id).await?.is_none() {
        return Err(configuration_error(format!(
            "authorization policy `{}` has not been bootstrapped",
            policy_id.as_str()
        )));
    }
    let authorize = Arc::new(AuthorizeOperationUseCase::new(
        policy_id,
        store,
        clock,
        AuthorizationDecisionTtl::from_seconds(60)?,
    ));
    Ok(Arc::new(GrpcAuthorizationGate::mutual_tls(
        authorize, principals,
    )))
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
