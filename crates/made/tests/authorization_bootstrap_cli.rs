use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use made_adapters::sqlite::SqliteAuthorizationPolicyStore;
use made_core::ports::AuthorizationPolicyStorePort;
use made_core::value_objects::{AuthenticationMethod, AuthorizationPolicyId, PrincipalKind};
use serde_json::Value;
use uuid::Uuid;

#[tokio::test]
async fn sqlite_bootstrap_is_explicit_idempotent_and_owner_bound() {
    let scratch = scratch_directory();
    std::fs::create_dir_all(&scratch).unwrap();
    let store = scratch.join("made.db");

    let first = invoke(&store, "policy-a", "host-a");
    assert!(first.status.success(), "{first:?}");
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(first["policy_id"], "policy-a");
    assert_eq!(first["owner_id"], "host-a");
    assert_eq!(first["version"], 1);
    assert_eq!(first["existing"], false);
    assert!(first.get("postgres_url").is_none());
    let snapshot = SqliteAuthorizationPolicyStore::open(&store)
        .unwrap()
        .load(&AuthorizationPolicyId::new("policy-a").unwrap())
        .await
        .unwrap()
        .unwrap();
    let owner = snapshot.policy.owner().unwrap();
    assert_eq!(owner.kind(), PrincipalKind::TrustedHost);
    assert_eq!(owner.method(), AuthenticationMethod::MutualTls);

    let repeated = invoke(&store, "policy-a", "host-a");
    assert!(repeated.status.success(), "{repeated:?}");
    let repeated: Value = serde_json::from_slice(&repeated.stdout).unwrap();
    assert_eq!(repeated["version"], 1);
    assert_eq!(repeated["existing"], true);

    let contradictory = invoke(&store, "policy-a", "another-host");
    assert!(!contradictory.status.success());
    let _ = std::fs::remove_dir_all(scratch);
}

#[tokio::test]
async fn sqlite_bootstrap_persists_explicit_separation_rules() {
    let scratch = scratch_directory();
    std::fs::create_dir_all(&scratch).unwrap();
    let store = scratch.join("made.db");
    let rules = scratch.join("separation.json");
    std::fs::write(
        &rules,
        r#"[{"approval_action":"approve_ceremony_guard","execution_action":"apply_ceremony_transition"}]"#,
    )
    .unwrap();

    let output = bootstrap_command()
        .env("MADE_CEREMONY_STORE_PATH", &store)
        .args([
            "bootstrap-authorization",
            "--policy-id",
            "separated",
            "--trusted-host-id",
            "host-a",
            "--separation-rules",
            rules.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let receipt: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(receipt["separation_rules"], 1);
    let snapshot = SqliteAuthorizationPolicyStore::open(&store)
        .unwrap()
        .load(&AuthorizationPolicyId::new("separated").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.policy.separation_rules().count(), 1);
    let _ = std::fs::remove_dir_all(scratch);
}

fn invoke(store: &Path, policy_id: &str, owner_id: &str) -> Output {
    bootstrap_command()
        .env("MADE_CEREMONY_STORE_PATH", store)
        .args([
            "bootstrap-authorization",
            "--policy-id",
            policy_id,
            "--trusted-host-id",
            owner_id,
        ])
        .output()
        .unwrap()
}

fn scratch_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tmp")
        .join(format!("authorization-bootstrap-{}", Uuid::new_v4()))
}

fn bootstrap_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_made"));
    command
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap());
    // Keep product configuration isolated while collecting child-process coverage.
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command
}
