use std::process::Command;

use super::{budget_planner_from_env, WorkerConnectorConfig, WorkerDaemonConfig};

const CASE: &str = "MADE_WORKER_CONFIGURATION_TEST_CASE";

#[test]
fn connector_configuration_is_complete_and_isolated() {
    if let Ok(case) = std::env::var(CASE) {
        check_configuration(&case);
        return;
    }
    for case in [
        "oci",
        "git",
        "http",
        "disabled",
        "unknown_connector",
        "missing_repository",
        "invalid_boolean",
        "invalid_number",
        "zero_heartbeat",
        "heartbeat_exceeds_lease",
        "invalid_budget",
        "zero_budget_version",
        "no_budget",
        "valid_budget",
    ] {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child.args([
            "--exact",
            "workers::worker_configuration_tests::connector_configuration_is_complete_and_isolated",
            "--nocapture",
        ]);
        for (name, _) in std::env::vars().filter(|(name, _)| name.starts_with("MADE_")) {
            child.env_remove(name);
        }
        child.envs([
            (CASE, case),
            ("MADE_WORKER_ENABLED", "true"),
            ("MADE_WORKER_CONNECTOR", "git"),
            ("MADE_WORKER_OWNER_ID", "owner"),
            ("MADE_WORKER_PRINCIPAL_ID", "worker"),
            ("MADE_WORKER_CAPACITY_DIRECTORY", "capacity"),
            ("MADE_WORKER_GIT_REPOSITORY", "repository"),
            ("MADE_WORKER_GIT_SCRATCH", "scratch"),
        ]);
        configure_case(&mut child, case);
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn configure_case(child: &mut Command, case: &str) {
    match case {
        "oci" => {
            child.envs([
                ("MADE_WORKER_CONNECTOR", "oci"),
                ("MADE_WORKER_OCI_IMAGE", "pinned-image"),
                ("MADE_WORKER_OCI_WORKSPACE", "workspace"),
                ("MADE_WORKER_OPERATION_ROOT", "operations"),
                ("MADE_WORKER_OCI_UID", "1001"),
                ("MADE_WORKER_OCI_NETWORK", "none"),
            ]);
        }
        "http" => {
            child.envs([
                ("MADE_WORKER_CONNECTOR", "http"),
                ("MADE_WORKER_HTTP_BASE", "http://127.0.0.1:8080"),
                ("MADE_WORKER_OPERATION_ROOT", "operations"),
                ("MADE_WORKER_CONNECTOR_TIMEOUT_MS", "1500"),
            ]);
        }
        // Settings for an unused connector cannot break the selected connector.
        "git" => {
            child.env("MADE_WORKER_OCI_CPUS", "invalid-unused-value");
        }
        "disabled" => {
            child
                .env("MADE_WORKER_ENABLED", "false")
                .env_remove("MADE_WORKER_CONNECTOR");
        }
        "unknown_connector" => {
            child.env("MADE_WORKER_CONNECTOR", "unknown");
        }
        "missing_repository" => {
            child.env_remove("MADE_WORKER_GIT_REPOSITORY");
        }
        "invalid_boolean" => {
            child.env("MADE_WORKER_ENABLED", "maybe");
        }
        "invalid_number" => {
            child.env("MADE_WORKER_WEIGHT", "negative");
        }
        "zero_heartbeat" => {
            child.env("MADE_WORKER_HEARTBEAT_MS", "0");
        }
        "heartbeat_exceeds_lease" => {
            child.env("MADE_WORKER_HEARTBEAT_MS", "30000");
        }
        "invalid_budget" => {
            child.env("MADE_WORKER_BUDGET_POLICY_VERSION", "1");
        }
        "zero_budget_version" | "valid_budget" => {
            child.envs([
                (
                    "MADE_WORKER_BUDGET_POLICY_VERSION",
                    if case == "valid_budget" { "1" } else { "0" },
                ),
                ("MADE_WORKER_BUDGET_MAX_DURATION_MICROS", "1000"),
                ("MADE_WORKER_BUDGET_MAX_TOKENS", "10"),
                ("MADE_WORKER_BUDGET_MAX_COST_MICROS", "0"),
                ("MADE_WORKER_BUDGET_MAX_TOOL_CALLS", "1"),
            ]);
        }
        "no_budget" => {}
        _ => panic!("unknown test case"),
    }
}

fn check_configuration(case: &str) {
    if case.contains("budget") {
        let planner = budget_planner_from_env();
        assert_eq!(
            planner.is_ok(),
            matches!(case, "no_budget" | "valid_budget")
        );
        return;
    }
    let parsed = WorkerDaemonConfig::from_env();
    if case == "disabled" {
        assert!(parsed.unwrap().is_none());
        return;
    }
    if !matches!(case, "git" | "oci" | "http") {
        assert!(
            parsed.is_err(),
            "invalid worker configuration must fail closed"
        );
        return;
    }
    let config = parsed.unwrap().unwrap();
    assert_eq!(config.owner.as_str(), "owner");
    assert_eq!(config.principal.as_str(), "worker");
    assert_eq!(config.connector.id(), case);
    assert!(config.heartbeat.as_millis() < u128::from(config.lease_ttl.get()));
    match config.connector {
        WorkerConnectorConfig::Git(git) => {
            assert_eq!(git.repository.to_str(), Some("repository"));
            assert_eq!(git.scratch.to_str(), Some("scratch"));
        }
        WorkerConnectorConfig::Http(http) => {
            assert_eq!(http.base_url, "http://127.0.0.1:8080");
            assert_eq!(http.timeout.as_millis(), 1500);
        }
        WorkerConnectorConfig::Oci(oci) => {
            assert_eq!(oci.uid, 1001);
            assert_eq!(oci.network, "none");
            assert_eq!(oci.workspace.to_str(), Some("workspace"));
            assert!(oci.memory_bytes > 0 && oci.pids > 0);
        }
    }
}
