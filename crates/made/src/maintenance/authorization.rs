use std::sync::Arc;

use anyhow::{bail, Context, Result};
use made_adapters::clock::SystemClock;
use made_adapters::config::ServiceConfig;
use made_adapters::postgres::{PostgresAuthorizationPolicyStore, PostgresConfig, PostgresPool};
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::{AuthorizationGateOutcome, AuthorizeOperationUseCase};
use made_core::ports::AuthorizationPolicyStorePort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationDecisionTtl, AuthorizationPolicyId,
    AuthorizationRequest, AuthorizationRequestId, AuthorizationScope, AuthorizationTargetDigest,
    AuthorizedOperation, PrincipalId, PrincipalKind,
};

use super::maintenance_request::MaintenanceRequest;

pub(super) async fn authorize(
    config: &ServiceConfig,
    request: &MaintenanceRequest,
    target: AuthorizationTargetDigest,
) -> Result<AuthorizedOperation> {
    let policy = AuthorizationPolicyId::new(required("MADE_AUTH_POLICY_ID")?)?;
    let principal = AuthenticatedPrincipal::new(
        PrincipalId::new(required("MADE_AUTH_TRUSTED_HOST_ID")?)?,
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )?;
    let store: Arc<dyn AuthorizationPolicyStorePort> = if let Some(url) = &config.postgres_url {
        let pool = PostgresPool::connect(&PostgresConfig::from_url(url.clone())).await?;
        Arc::new(PostgresAuthorizationPolicyStore::new(pool))
    } else {
        let path = config
            .ceremony_store_path
            .as_ref()
            .context("MADE_CEREMONY_STORE_PATH is required")?;
        if !std::path::Path::new(path).is_file() {
            bail!("authorization database must already exist");
        }
        Arc::new(SqliteAuthorizationPolicyStore::open(path)?)
    };
    let authorize = AuthorizeOperationUseCase::new(
        policy,
        store,
        Arc::new(SystemClock::new()),
        AuthorizationDecisionTtl::from_seconds(60)?,
    );
    let mut intent = AuthorizationRequest::new(
        AuthorizationRequestId::new(format!("maintenance:{}", uuid::Uuid::new_v4()))?,
        principal.clone(),
        request.command.action(),
        AuthorizationScope::Global,
        target,
    );
    if let Some(approval) = &request.approval_decision_id {
        intent = intent.with_approval(approval.clone());
    }
    match authorize.execute(intent).await? {
        AuthorizationGateOutcome::Allowed { evidence, .. } => {
            Ok(AuthorizedOperation::new(principal, evidence)?)
        }
        AuthorizationGateOutcome::Denied { .. } => {
            bail!("maintenance denied by the configured policy")
        }
        AuthorizationGateOutcome::Expired { .. } => {
            bail!("maintenance authorization expired before admission")
        }
    }
}

fn required(name: &str) -> Result<String> {
    std::env::var(name)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .with_context(|| format!("{name} is required in trusted host configuration"))
}
