mod execution_support;

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;

use execution_support::{request, scratch};
use made_adapters::connectors::GitExecutionConnector;
use made_core::ports::{
    CeremonyExecutionConnectorOutcome as Outcome, CeremonyExecutionConnectorPort,
    ExecutionCancellation,
};
use made_core::value_objects::ExecutionConnectorId;
use sha2::{Digest, Sha256};

fn git(repository: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    let mut command = Command::new("git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Execution test")
        .env("GIT_AUTHOR_EMAIL", "execution-test@localhost")
        .env("GIT_COMMITTER_NAME", "Execution test")
        .env("GIT_COMMITTER_EMAIL", "execution-test@localhost")
        .arg("--git-dir")
        .arg(repository)
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        use std::io::Write;
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn repository(root: &Path) -> (std::path::PathBuf, String) {
    let repository = root.join("acceptance.git");
    assert!(Command::new("git")
        .args(["init", "--bare"])
        .arg(&repository)
        .status()
        .unwrap()
        .success());
    let blob = git(
        &repository,
        &["hash-object", "-w", "--stdin"],
        Some(b"base\n"),
    );
    let tree_input = format!("100644 blob {blob}\tfile.txt\n");
    let tree = git(&repository, &["mktree"], Some(tree_input.as_bytes()));
    let tip = git(&repository, &["commit-tree", &tree], Some(b"base\n"));
    git(&repository, &["update-ref", "refs/heads/main", &tip], None);
    (repository, tip)
}

fn change(repository: &Path, tip: &str, replacement: &str) -> String {
    let worktree = repository
        .parent()
        .unwrap()
        .join(format!("worktree-{replacement}"));
    assert!(Command::new("git")
        .args(["clone", "--quiet"])
        .arg(repository)
        .arg(&worktree)
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .arg("-C")
        .arg(&worktree)
        .args(["checkout", "--quiet", tip])
        .status()
        .unwrap()
        .success());
    std::fs::write(worktree.join("file.txt"), format!("{replacement}\n")).unwrap();
    let output = Command::new("git")
        .arg("-C")
        .arg(&worktree)
        .args(["diff", "--binary", "--", "file.txt"])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn operation(
    repository: &Path,
    tip: &str,
    diff: &str,
) -> made_core::ports::CeremonyExecutionRequest {
    request(
        "git",
        serde_json::json!({"git": {
            "repository": repository,
            "branch": "refs/heads/main",
            "expected_tip": tip,
            "diff": diff,
            "diff_sha256": format!("{:x}", Sha256::digest(diff.as_bytes())),
        }}),
    )
}

fn observed(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Observed(_))
}

#[tokio::test]
async fn applies_one_atomic_change_and_recovers_from_git_metadata() {
    let root = scratch();
    let (repository, tip) = repository(root.path());
    let diff = change(&repository, &tip, "accepted");
    let request = operation(&repository, &tip, &diff);
    let connector = GitExecutionConnector::new(
        ExecutionConnectorId::new("git").unwrap(),
        &repository,
        root.path().join("scratch"),
    )
    .unwrap();

    assert!(observed(
        &connector.execute_or_recover(request.clone()).await.unwrap()
    ));
    let advanced = git(&repository, &["rev-parse", "refs/heads/main"], None);
    assert_ne!(advanced, tip);

    let recovered = GitExecutionConnector::new(
        ExecutionConnectorId::new("git").unwrap(),
        &repository,
        root.path().join("other-scratch"),
    )
    .unwrap()
    .recover_intent(request.intent())
    .await
    .unwrap();
    assert!(observed(&recovered));
}

#[tokio::test]
async fn expected_tip_transaction_allows_only_one_competing_change() {
    let root = scratch();
    let (repository, tip) = repository(root.path());
    let first = operation(&repository, &tip, &change(&repository, &tip, "first"));
    let second = operation(&repository, &tip, &change(&repository, &tip, "second"));
    let connector = Arc::new(
        GitExecutionConnector::new(
            ExecutionConnectorId::new("git").unwrap(),
            &repository,
            root.path().join("scratch"),
        )
        .unwrap(),
    );

    let (left, right) = tokio::join!(
        connector.execute_or_recover(first),
        connector.execute_or_recover(second),
    );
    assert_eq!(
        usize::from(left.as_ref().is_ok_and(observed))
            + usize::from(right.as_ref().is_ok_and(observed)),
        1
    );
}

#[tokio::test]
async fn rejects_digest_mismatch_and_cancelled_authority_before_publication() {
    let root = scratch();
    let (repository, tip) = repository(root.path());
    let diff = change(&repository, &tip, "cancelled");
    let connector = GitExecutionConnector::new(
        ExecutionConnectorId::new("git").unwrap(),
        &repository,
        root.path().join("scratch"),
    )
    .unwrap();
    let cancellation = ExecutionCancellation::new();
    cancellation.cancel();
    assert!(connector
        .execute_cancellable(operation(&repository, &tip, &diff), cancellation)
        .await
        .is_err());
    assert_eq!(
        git(&repository, &["rev-parse", "refs/heads/main"], None),
        tip
    );

    let bad = request(
        "git",
        serde_json::json!({"git": {
            "repository": repository,
            "branch": "refs/heads/main",
            "expected_tip": tip,
            "diff": diff,
            "diff_sha256": "0".repeat(64),
        }}),
    );
    assert!(connector.execute_or_recover(bad).await.is_err());
}
