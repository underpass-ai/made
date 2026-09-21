//! Discovery tells a host which activation adapter it is really behind.
//!
//! The question a host asks with `made_discover_capabilities` is
//! whether waiting to be woken is a plan. A constant in the source
//! would answer `none` forever, including on the deployment whose
//! operator had just configured a command, so the answer is taken from
//! the port the engine actually composed — whether that port arrived
//! from the environment or from a host wiring its own.

use std::sync::Arc;
use std::time::Duration;

use made_adapters::activation::{
    select_host_activation, CommandHostActivation, HostActivationCommand, COMMAND_ENV,
};
use made_core::ports::HostActivationPort;
use made_embedded::EmbeddedMade;
use made_mcp::{
    EmbeddedMadeMcpBackend, GrpcMadeMcpBackend, MadeMcpGrpcTlsConfig, MadeMcpServer,
    MadeMcpToolBackend,
};
use serde_json::{json, Value};

async fn adapter_reported_by(server: &MadeMcpServer) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "made_discover_capabilities", "arguments": {}}});
    let response = server.handle_json_line(&request.to_string()).await.unwrap();
    let result = serde_json::from_str::<Value>(&response).unwrap();
    result["result"]["structuredContent"]["host_activation"]["adapter"].clone()
}

/// A runnable no-op standing in for a host's own activation script.
fn a_host_script(name: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let script = std::env::temp_dir().join(format!("made-host-activation-{name}.sh"));
    std::fs::write(&script, "#!/bin/sh\ncat > /dev/null\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn a_command_adapter() -> Arc<dyn HostActivationPort> {
    Arc::new(CommandHostActivation::new(
        HostActivationCommand::new(a_host_script("wired"), []).unwrap(),
        Duration::from_secs(1),
        1024,
    ))
}

#[tokio::test]
async fn a_deployment_without_an_adapter_is_reported_as_none() {
    let server = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(EmbeddedMade::default()));

    assert_eq!(adapter_reported_by(&server).await, "none");
}

#[tokio::test]
async fn a_host_that_wires_its_own_port_is_reported_as_what_it_wired() {
    let made = EmbeddedMade::builder()
        .with_host_activation(a_command_adapter())
        .build();
    let server = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(made));

    assert_eq!(adapter_reported_by(&server).await, "command");
}

#[tokio::test]
async fn a_backend_that_composes_no_engine_answers_for_itself() {
    let server = MadeMcpServer::with_backend(GrpcMadeMcpBackend::new(
        "http://127.0.0.1:1".to_owned(),
        MadeMcpGrpcTlsConfig::disabled(),
    ));

    assert_eq!(adapter_reported_by(&server).await, "none");
}

/// The environment selects the adapter, and discovery follows it.
///
/// One test rather than three because `MADE_HOST_ACTIVATION_COMMAND` is
/// process-wide: two of these running at once would each see the
/// other's setting. It ends where it started, with the variable unset
/// and the selection back to `none`.
#[tokio::test]
async fn the_environment_selects_the_adapter_discovery_reports() {
    assert_eq!(select_host_activation().unwrap().kind().as_str(), "none");

    std::env::set_var(COMMAND_ENV, "made-no-such-activation-command");
    let refused = select_host_activation();
    std::env::set_var(COMMAND_ENV, a_host_script("from-env").display().to_string());
    let selected = select_host_activation().unwrap();
    let made = EmbeddedMade::builder()
        .with_host_activation(selected.clone())
        .build();
    let server = MadeMcpServer::with_backend(EmbeddedMadeMcpBackend::new(made));
    let reported = adapter_reported_by(&server).await;
    std::env::remove_var(COMMAND_ENV);

    assert!(
        refused.is_err(),
        "a command that resolves nowhere must not degrade into `none`"
    );
    assert_eq!(selected.kind().as_str(), "command");
    assert_eq!(reported, "command");
    assert_eq!(select_host_activation().unwrap().kind().as_str(), "none");
}

/// A default the boxed holder forgets to forward is a silent wrong answer.
///
/// `Arc<T>` implements the backend trait itself, so a call through the
/// server's `Arc<dyn ...>` resolves to that forwarding impl rather than
/// to the backend inside it. A default left unforwarded there looks
/// exactly like a deployment that composed nothing.
#[test]
fn the_answer_survives_the_boxed_holder() {
    let made = EmbeddedMade::builder()
        .with_host_activation(a_command_adapter())
        .build();
    let held: Arc<dyn MadeMcpToolBackend> = Arc::new(EmbeddedMadeMcpBackend::new(made));

    assert_eq!(held.host_activation_adapter(), "command");
}
