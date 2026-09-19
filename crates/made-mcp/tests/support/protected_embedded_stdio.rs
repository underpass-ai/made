use std::path::Path;
use std::sync::Arc;

use made_adapters::clock::SystemClock;
use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_app::authorization::AuthorizationPolicyAdministrationService;
use made_core::value_objects::{
    AuthenticatedPrincipal, AuthenticationMethod, AuthorizationAction, AuthorizationGrant,
    AuthorizationGrantId, AuthorizationGrantIssuer, AuthorizationPolicyId, AuthorizationScope,
    DelegationDepth, PrincipalId, PrincipalKind,
};
use tokio::process::Command;

pub async fn command(directory: &Path, actions: &[AuthorizationAction]) -> Command {
    let path = directory.join("ceremonies.sqlite3");
    let policy = "embedded-stdio-policy";
    let host = "embedded-stdio-host";
    let administration = AuthorizationPolicyAdministrationService::new(
        AuthorizationPolicyId::new(policy).unwrap(),
        Arc::new(SqliteAuthorizationPolicyStore::open(&path).unwrap()),
        Arc::new(SystemClock::new()),
    );
    let owner = AuthenticatedPrincipal::new(
        PrincipalId::new(host).unwrap(),
        PrincipalKind::TrustedHost,
        AuthenticationMethod::LocalHostPolicy,
    )
    .unwrap();
    administration
        .open(owner.clone(), Vec::new())
        .await
        .unwrap();
    administration
        .issue(
            &owner,
            AuthorizationGrant::new(
                AuthorizationGrantId::new("stdio-fixture-actions").unwrap(),
                owner.id().clone(),
                actions.iter().copied(),
                AuthorizationScope::Global,
                (time::OffsetDateTime::UNIX_EPOCH, None),
                DelegationDepth::none(),
                AuthorizationGrantIssuer::direct(owner.clone()),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_made-mcp"));
    command
        .env("MADE_MCP_BACKEND", "embedded")
        .env("MADE_MCP_STORE_PATH", path)
        .env("MADE_AUTH_POLICY_ID", policy)
        .env("MADE_AUTH_TRUSTED_HOST_ID", host)
        .env("MADE_CEREMONY_STORE_ID", "embedded-stdio-test-store")
        .env("MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY", "a5".repeat(32));
    command
}
