mod local_execution_adapter;
mod local_execution_config;
mod local_execution_error;
mod local_execution_mode;
mod local_execution_outcome;
mod local_execution_port;
mod local_execution_receipt;
mod local_execution_request;
mod local_network_policy;
mod local_process_termination;

pub use local_execution_adapter::LocalExecutionAdapter;
pub use local_execution_config::LocalExecutionConfig;
pub use local_execution_error::LocalExecutionError;
pub use local_execution_mode::LocalExecutionMode;
pub use local_execution_outcome::LocalExecutionOutcome;
pub use local_execution_port::LocalExecutionPort;
pub use local_execution_receipt::LocalExecutionReceipt;
pub use local_execution_request::LocalExecutionRequest;
pub use local_network_policy::LocalNetworkPolicy;
pub use local_process_termination::LocalProcessTermination;

pub use LocalExecutionAdapter as LocalExecutionConnector;
pub use LocalExecutionConfig as LocalExecutionLimits;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;

    fn script(root: &TempDir, body: &str) -> PathBuf {
        let path = root.path().join("run.sh");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn adapter(root: &TempDir, timeout: Duration, output_limit: usize) -> LocalExecutionAdapter {
        LocalExecutionAdapter::new(
            LocalExecutionConfig::trusted_local(
                root.path(),
                LocalNetworkPolicy::NotAllowed,
                timeout,
                output_limit,
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_executable_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let error = adapter(&root, Duration::from_secs(1), 1024)
            .execute(LocalExecutionRequest::new(script(&outside, "exit 0")))
            .await
            .unwrap_err();
        assert!(matches!(error, LocalExecutionError::ExecutableOutsideRoot));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_returns_structured_receipt_and_terminates_group() {
        let root = tempfile::tempdir().unwrap();
        let receipt = adapter(&root, Duration::from_millis(50), 1024)
            .execute(LocalExecutionRequest::new(script(&root, "sleep 5")))
            .await
            .unwrap();
        assert_eq!(receipt.outcome, LocalExecutionOutcome::TimedOut);
        assert!(receipt.process_termination.requested);
        assert!(receipt.process_termination.attempted);
        assert!(receipt.process_termination.succeeded);
        assert_eq!(receipt.network, LocalNetworkPolicy::NotAllowed);
        assert_eq!(receipt.mode, LocalExecutionMode::TrustedLocal);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn output_limit_returns_counts_without_output_content() {
        let root = tempfile::tempdir().unwrap();
        let receipt = adapter(&root, Duration::from_secs(1), 4)
            .execute(
                LocalExecutionRequest::new(script(
                    &root,
                    "printf 'super-secret-output-123456789'; sleep 5",
                ))
                .with_args(["secret-argument"])
                .with_environment("TOKEN", "secret-environment-value"),
            )
            .await
            .unwrap();
        assert_eq!(receipt.outcome, LocalExecutionOutcome::OutputLimitExceeded);
        assert!(receipt.output_bytes > receipt.max_output_bytes);
        assert!(receipt.process_termination.succeeded);
        let serialized = serde_json::to_string(&receipt).unwrap();
        assert!(!serialized.contains("super-secret-output"));
        assert!(!serialized.contains("secret-argument"));
        assert!(!serialized.contains("secret-environment-value"));
    }

    #[test]
    fn unsupported_isolated_mode_is_explicitly_rejected() {
        let root = tempfile::tempdir().unwrap();
        let error = LocalExecutionConfig::new(
            LocalExecutionMode::IsolatedLinuxUnavailable,
            root.path(),
            LocalNetworkPolicy::Allowed,
            Duration::from_secs(1),
            1024,
        )
        .unwrap_err();
        assert!(matches!(error, LocalExecutionError::UnsupportedMode { .. }));
        assert_eq!(
            serde_json::to_string(&LocalExecutionMode::IsolatedLinuxUnavailable).unwrap(),
            "\"isolated-linux-unavailable\""
        );
    }

    #[test]
    fn invalid_limits_and_root_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            LocalExecutionConfig::trusted_local(
                root.path(),
                LocalNetworkPolicy::Allowed,
                Duration::ZERO,
                1
            ),
            Err(LocalExecutionError::InvalidTimeout)
        ));
        assert!(matches!(
            LocalExecutionConfig::trusted_local(
                root.path(),
                LocalNetworkPolicy::Allowed,
                Duration::from_secs(1),
                0
            ),
            Err(LocalExecutionError::InvalidOutputLimit)
        ));
        let file = root.path().join("not-a-directory");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(
            LocalExecutionConfig::trusted_local(
                &file,
                LocalNetworkPolicy::Allowed,
                Duration::from_secs(1),
                1
            ),
            Err(LocalExecutionError::FilesystemRootNotDirectory)
        ));
    }
}
