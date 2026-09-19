#![cfg(target_os = "linux")]
mod c6_execution_support;
use c6_execution_support::{request, scratch};
use made_adapters::execution::{OciExecutionConfig, OciExecutionConnector};
use made_core::{
    ports::{
        CeremonyExecutionConnectorOutcome as Outcome, CeremonyExecutionConnectorPort,
        ExecutionCancellation,
    },
    value_objects::ExecutionConnectorId,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    process::Command,
    sync::Arc,
    time::Duration,
};

fn image() -> String {
    std::env::var("MADE_OCI_TEST_IMAGE")
        .expect("run explicitly with an immutable MADE_OCI_TEST_IMAGE")
}
fn config(root: &std::path::Path) -> OciExecutionConfig {
    let workspace = root.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    fs::set_permissions(&workspace, fs::Permissions::from_mode(0o777)).unwrap();
    OciExecutionConfig {
        image: image(),
        workspace,
        operation_root: root.join("operations"),
        network: "none".into(),
        uid: 65534,
        cpus: 0.2,
        memory_bytes: 64 * 1024 * 1024,
        pids: 16,
        max_output_bytes: 4096,
        timeout: Duration::from_secs(10),
    }
}
fn name(request: &made_core::ports::CeremonyExecutionRequest) -> String {
    format!(
        "made-oci-{:x}",
        Sha256::digest(
            request
                .intent()
                .operation()
                .operation_id()
                .as_str()
                .as_bytes()
        )
    )
}
fn inspect(name: &str) -> serde_json::Value {
    let output = Command::new("docker")
        .args(["inspect", name])
        .output()
        .unwrap();
    assert!(output.status.success());
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()[0].clone()
}
fn success(outcome: Outcome) -> bool {
    let Outcome::Observed(observation) = outcome else {
        panic!("missing observation")
    };
    observation.into_parts().2.is_success()
}

struct ContainerGuard(String);
impl Drop for ContainerGuard {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
    }
}

struct NetworkGuard {
    network: String,
    service: String,
}
impl Drop for NetworkGuard {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.service])
            .output();
        let _ = Command::new("docker")
            .args(["network", "rm", &self.network])
            .output();
    }
}

#[tokio::test]
#[ignore = "requires Linux Docker and pinned image"]
async fn filesystem_identity_network_and_symlink_escape_are_enforced() {
    let root = scratch();
    let policy = config(root.path());
    fs::write(root.path().join("secret"), "host-secret").unwrap();
    symlink(root.path().join("secret"), policy.workspace.join("escape")).unwrap();
    let connector =
        OciExecutionConnector::new(ExecutionConnectorId::new("oci").unwrap(), policy).unwrap();
    let req = request(
        "oci",
        serde_json::json!({"oci":{"argv":["/bin/bash","-c","set -eu; test $(id -u) = 65534; ! touch /etc/escape; ! cat /workspace/escape; ! cat /workspace/../secret; ! test -e /var/run/docker.sock; ! bash -c 'echo bad > /dev/tcp/1.1.1.1/80'; grep -q 'CapEff:\\s*0000000000000000' /proc/self/status; grep -q 'NoNewPrivs:\\s*1' /proc/self/status; echo allowed > /workspace/allowed"]}}),
    );
    let container = name(&req);
    let _guard = ContainerGuard(container.clone());
    assert!(success(
        connector.execute_or_recover(req.clone()).await.unwrap()
    ));
    let state = inspect(&container);
    assert_eq!(state["HostConfig"]["NetworkMode"], "none");
    assert_eq!(state["HostConfig"]["ReadonlyRootfs"], true);
    assert_eq!(
        fs::read_to_string(root.path().join("secret")).unwrap(),
        "host-secret"
    );
    fs::remove_dir_all(root.path().join("operations")).unwrap();
    fs::create_dir(root.path().join("operations")).unwrap();
    assert!(success(
        connector.recover_intent(req.intent()).await.unwrap()
    ));
}

#[tokio::test]
#[ignore = "requires Linux Docker and pinned image"]
async fn timeout_cancellation_and_output_kill_whole_container() {
    for mode in ["timeout", "cancel", "output"] {
        let root = scratch();
        let mut policy = config(root.path());
        policy.timeout = Duration::from_millis(700);
        let connector =
            OciExecutionConnector::new(ExecutionConnectorId::new("oci").unwrap(), policy).unwrap();
        let script = if mode == "output" {
            "yes flood"
        } else {
            "sleep 120 & wait"
        };
        let req = request(
            "oci",
            serde_json::json!({"oci":{"argv":["/bin/sh","-c",script]}}),
        );
        let container = name(&req);
        let _guard = ContainerGuard(container.clone());
        let cancellation = ExecutionCancellation::new();
        let trigger = cancellation.clone();
        if mode == "cancel" {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(400)).await;
                trigger.cancel();
            });
        }
        assert!(!success(
            connector
                .execute_cancellable(req.clone(), cancellation)
                .await
                .unwrap()
        ));
        assert_eq!(inspect(&container)["State"]["Running"], false);
        assert!(!success(
            connector.recover_intent(req.intent()).await.unwrap()
        ));
    }
}

#[tokio::test]
#[ignore = "requires Linux Docker and pinned image"]
async fn memory_cpu_and_pid_limits_are_effective() {
    let root = scratch();
    let policy = config(root.path());
    let workspace = policy.workspace.clone();
    let connector =
        OciExecutionConnector::new(ExecutionConnectorId::new("oci").unwrap(), policy).unwrap();
    let req = request(
        "oci",
        serde_json::json!({"oci":{"argv":["/usr/bin/perl","-e","$x = 'x' x (256*1024*1024); sleep 30"]}}),
    );
    let container = name(&req);
    let _guard = ContainerGuard(container.clone());
    assert!(!success(connector.execute_or_recover(req).await.unwrap()));
    assert_eq!(inspect(&container)["State"]["OOMKilled"], true);
    let req = request(
        "oci",
        serde_json::json!({"oci":{"argv":["/bin/sh","-c","perl -e '$SIG{ALRM}=sub{exit};alarm 2;1 while 1'; cat /sys/fs/cgroup/cpu.stat > /workspace/cpu; perl -e 'for(1..100){$p=fork();if(!defined($p)){last}if(!$p){sleep 2;exit}$n++}open(F,\">/workspace/forks\");print F $n;close F;1 while wait()>0' "]}}),
    );
    let container = name(&req);
    let _guard = ContainerGuard(container.clone());
    assert!(success(connector.execute_or_recover(req).await.unwrap()));
    let cpu = fs::read_to_string(workspace.join("cpu")).unwrap();
    let throttled = cpu
        .lines()
        .find(|l| l.starts_with("nr_throttled "))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(throttled > 0);
    let forks = fs::read_to_string(workspace.join("forks"))
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(forks > 0 && forks < 16);
}

#[tokio::test]
#[ignore = "requires Linux Docker and pinned image"]
async fn explicit_isolated_network_reaches_only_the_acceptance_service() {
    let root = scratch();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let network = format!("made-c6-{suffix}");
    let service = format!("made-c6-service-{suffix}");
    assert!(Command::new("docker")
        .args(["network", "create", &network])
        .status()
        .unwrap()
        .success());
    let _network_guard = NetworkGuard {
        network: network.clone(),
        service: service.clone(),
    };
    let perl = r#"use IO::Socket::INET;$s=IO::Socket::INET->new(LocalPort=>8080,Listen=>5,Reuse=>1) or die $!;while($c=$s->accept){<$c>;while(<$c>){last if /^\r?$/}print $c "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";close $c}"#;
    assert!(Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            &service,
            "--network",
            &network,
            "--user",
            "65534:65534",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            "--read-only",
            &image(),
            "/usr/bin/perl",
            "-e",
            perl,
        ])
        .status()
        .unwrap()
        .success());
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut policy = config(root.path());
    policy.network = network;
    let connector =
        OciExecutionConnector::new(ExecutionConnectorId::new("oci").unwrap(), policy).unwrap();
    let script = format!("exec 3<>/dev/tcp/{service}/8080; printf 'GET / HTTP/1.1\\r\\nHost: {service}\\r\\nConnection: close\\r\\n\\r\\n' >&3; grep -q '^ok$' <&3");
    let request = request(
        "oci",
        serde_json::json!({"oci": {"argv": ["/bin/bash", "-c", script]}}),
    );
    let container = name(&request);
    let _container_guard = ContainerGuard(container);
    assert!(success(
        connector.execute_or_recover(request).await.unwrap()
    ));
}

#[tokio::test]
#[ignore = "requires Linux Docker and pinned image"]
async fn in_container_watchdog_bounds_runtime_after_host_task_dies() {
    let root = scratch();
    let mut policy = config(root.path());
    policy.timeout = Duration::from_millis(900);
    let connector = Arc::new(
        OciExecutionConnector::new(ExecutionConnectorId::new("oci").unwrap(), policy).unwrap(),
    );
    let request = request(
        "oci",
        serde_json::json!({"oci": {"argv": ["/bin/sh", "-c", "sleep 120"]}}),
    );
    let container = name(&request);
    let _guard = ContainerGuard(container.clone());
    let execution = Arc::clone(&connector);
    let running_request = request.clone();
    let task = tokio::spawn(async move { execution.execute_or_recover(running_request).await });
    for _ in 0..100 {
        if Command::new("docker")
            .args(["inspect", "--format", "{{.State.Running}}", &container])
            .output()
            .is_ok_and(|output| output.status.success() && output.stdout == b"true\n")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(inspect(&container)["State"]["Running"], true);
    task.abort();
    let _ = task.await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(inspect(&container)["State"]["Running"], false);
    assert!(!success(
        connector.recover_intent(request.intent()).await.unwrap()
    ));
}

#[test]
fn rejects_mutable_image_root_identity_and_unbounded_policy() {
    let root = scratch();
    std::env::var("MADE_OCI_TEST_IMAGE").ok();
    let workspace = root.path().join("workspace");
    fs::create_dir(&workspace).unwrap();
    let policy = OciExecutionConfig {
        image: "ubuntu:noble".into(),
        workspace,
        operation_root: root.path().join("ops"),
        network: "host".into(),
        uid: 0,
        cpus: 0.0,
        memory_bytes: 0,
        pids: 0,
        max_output_bytes: 0,
        timeout: Duration::ZERO,
    };
    assert!(OciExecutionConnector::new(ExecutionConnectorId::new("oci").unwrap(), policy).is_err());
}
