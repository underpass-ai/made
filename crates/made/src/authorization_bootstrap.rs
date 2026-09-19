use std::sync::Arc;

use anyhow::{bail, Context, Result};
use made_adapters::clock::SystemClock;
use made_adapters::config::EnvConfiguration;
use made_adapters::postgres::{PostgresAuthorizationPolicyStore, PostgresConfig, PostgresPool};
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::{
    AuthorizationMutationOutcome, AuthorizationPolicyAdministrationService,
};
use made_core::ports::AuthorizationPolicyStorePort;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationPolicyId, PrincipalId, PrincipalKind,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
struct AuthorizationBootstrapReceipt {
    policy_id: String,
    version: u64,
    existing: bool,
    owner_id: String,
}

pub(crate) async fn run(arguments: Vec<String>) -> Result<()> {
    let (policy_id, owner_id) = parse_arguments(&arguments)?;
    let config = EnvConfiguration::new().load()?;
    let store = policy_store(&config).await?;
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new(owner_id.clone())?,
        PrincipalKind::TrustedHost,
        AuthenticationMethod::MutualTls,
    )?;
    let administration = AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new(policy_id.clone())?,
        store,
        Arc::new(SystemClock::new()),
    );
    let (version, existing) = match administration.open(owner, Vec::new()).await? {
        AuthorizationMutationOutcome::Applied { version } => (version.value(), false),
        AuthorizationMutationOutcome::Existing { version } => (version.value(), true),
    };
    let receipt = AuthorizationBootstrapReceipt {
        policy_id,
        version,
        existing,
        owner_id,
    };
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}

fn parse_arguments(arguments: &[String]) -> Result<(String, String)> {
    let mut policy_id = None;
    let mut owner_id = None;
    let mut index = 0;
    while index < arguments.len() {
        let target = match arguments[index].as_str() {
            "--policy-id" => &mut policy_id,
            "--trusted-host-id" => &mut owner_id,
            unexpected => bail!("unexpected bootstrap-authorization argument `{unexpected}`"),
        };
        index += 1;
        let value = arguments
            .get(index)
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .context("bootstrap-authorization option requires a non-empty value")?;
        if target.replace(value).is_some() {
            bail!("bootstrap-authorization option was supplied more than once");
        }
        index += 1;
    }
    Ok((
        policy_id.context("--policy-id is required")?,
        owner_id.context("--trusted-host-id is required")?,
    ))
}

async fn policy_store(
    config: &made_adapters::config::ServiceConfig,
) -> Result<Arc<dyn AuthorizationPolicyStorePort>> {
    if let Some(url) = &config.postgres_url {
        let pool = PostgresPool::connect(&PostgresConfig::from_url(url.clone())).await?;
        pool.run_migrations().await?;
        return Ok(Arc::new(PostgresAuthorizationPolicyStore::new(pool)));
    }
    if let Some(path) = &config.ceremony_store_path {
        return Ok(Arc::new(SqliteAuthorizationPolicyStore::open(path)?));
    }
    bail!("bootstrap-authorization requires MADE_POSTGRES_URL or MADE_CEREMONY_STORE_PATH")
}

#[cfg(test)]
mod tests {
    use super::parse_arguments;

    #[test]
    fn arguments_require_each_explicit_identity_once() {
        let parsed = parse_arguments(&[
            "--trusted-host-id".to_owned(),
            "host-a".to_owned(),
            "--policy-id".to_owned(),
            "policy-a".to_owned(),
        ])
        .unwrap();
        assert_eq!(parsed, ("policy-a".to_owned(), "host-a".to_owned()));
        assert!(parse_arguments(&["--policy-id".to_owned(), "policy-a".to_owned()]).is_err());
        assert!(parse_arguments(&[
            "--policy-id".to_owned(),
            "policy-a".to_owned(),
            "--policy-id".to_owned(),
            "policy-a".to_owned(),
            "--trusted-host-id".to_owned(),
            "host-a".to_owned(),
        ])
        .is_err());
    }
}
