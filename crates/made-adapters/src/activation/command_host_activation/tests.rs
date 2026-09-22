use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use made_core::value_objects::{
    AttentionKind, AttentionReason, CeremonyId, CeremonyInterventionId, HostActivationMode,
    HostAddress, HostAgentIncarnation, HostDeliveryItem, HostDeliveryPolicy, HostDeliveryTarget,
    HostDestination, HostKind, IntegratorBindingId, IntegratorFence, RoleId,
};
use time::OffsetDateTime;

use super::*;

/// A host stand-in, run as `sh -c <body> <dir> --one`.
///
/// The body is an argument rather than a file this module wrote and
/// then executed. These tests leave children alive on purpose, and a
/// fork in one test inherits, for the instant before it execs, every
/// descriptor another test has open — including the one it is writing
/// its script through. Exec'ing a file somebody still holds open for
/// writing is `ETXTBSY`, so a written script makes the suite flaky in
/// proportion to how well it tests. `$0` is the directory the body
/// writes its evidence into.
struct HostScript {
    directory: tempfile::TempDir,
    body: String,
}

impl HostScript {
    fn new(body: &str) -> Self {
        Self {
            directory: tempfile::tempdir().unwrap(),
            body: body.to_owned(),
        }
    }

    fn adapter(&self) -> CommandHostActivation {
        self.adapter_with(Duration::from_secs(5), 65_536)
    }

    fn adapter_with(&self, timeout: Duration, max_output: usize) -> CommandHostActivation {
        CommandHostActivation::new(
            HostActivationCommand::new(
                "/bin/sh",
                [
                    "-c".to_owned(),
                    self.body.clone(),
                    self.directory.path().display().to_string(),
                    "--one".to_owned(),
                ],
            )
            .unwrap(),
            timeout,
            max_output,
        )
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.directory.path().join(name)).unwrap_or_default()
    }
}

/// A script this module really writes, for the two cases about files.
fn a_written_script(directory: &tempfile::TempDir) -> PathBuf {
    let path = directory.path().join("activate.sh");
    fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn binding() -> IntegratorBinding {
    IntegratorBinding::new(
        IntegratorBindingId::new("b-1").unwrap(),
        made_core::value_objects::IntegratorScope::ceremony(CeremonyId::new("c-1").unwrap()),
        RoleId::new("INTEGRATOR").unwrap(),
        HostDestination::new(
            HostKind::new("claude-code").unwrap(),
            HostAddress::new("session-42").unwrap(),
            HostActivationMode::Command,
        ),
        HostAgentIncarnation::new("run-1").unwrap(),
        OffsetDateTime::UNIX_EPOCH,
    )
}

fn record() -> HostDeliveryRecord {
    HostDeliveryRecord::queued(
        item(),
        HostDeliveryTarget::integrator_binding(IntegratorBindingId::new("b-1").unwrap()),
        HostDeliveryPolicy::activation(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .unwrap()
}

fn item() -> HostDeliveryItem {
    HostDeliveryItem::intervention(
        CeremonyId::new("c-1").unwrap(),
        CeremonyInterventionId::new("i-1").unwrap(),
    )
}

fn envelope() -> HostActivationEnvelope {
    HostActivationEnvelope::new(
        record().id().clone(),
        IntegratorBindingId::new("b-1").unwrap(),
        IntegratorFence::FIRST,
        item(),
        CeremonyId::new("c-1").unwrap(),
        AttentionKind::InterventionRequested,
        AttentionReason::new("a supervisor asked a question").unwrap(),
        OffsetDateTime::UNIX_EPOCH,
    )
}

async fn activate(adapter: &CommandHostActivation) -> HostActivationOutcome {
    adapter
        .activate(&binding(), &record(), &envelope())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_command_that_exits_clean_is_a_transport_receipt() {
    let script = HostScript::new("cat > /dev/null; echo woke-session-42");

    let outcome = activate(&script.adapter()).await;

    let HostActivationOutcome::Accepted(receipt) = outcome else {
        panic!("a clean exit is an accepted hand-off, got {outcome:?}");
    };
    assert_eq!(receipt.adapter(), HostActivationAdapterKind::Command);
    assert_eq!(
        receipt.transport_ref().map(HostTransportRef::as_str),
        Some("woke-session-42")
    );
}

#[tokio::test]
async fn a_command_that_exits_dirty_is_a_failure_that_says_why() {
    let script = HostScript::new("cat > /dev/null; echo 'no such session' >&2; exit 3");

    let outcome = activate(&script.adapter()).await;

    let HostActivationOutcome::Failed(reason) = outcome else {
        panic!("a non-zero exit is a failed hand-off, got {outcome:?}");
    };
    assert!(reason.as_str().contains("exited 3"), "{reason}");
    assert!(reason.as_str().contains("no such session"), "{reason}");
}

#[tokio::test]
async fn a_command_that_never_answers_fails_on_the_timeout() {
    let script = HostScript::new("sleep 30");

    let outcome = activate(&script.adapter_with(Duration::from_millis(150), 65_536)).await;

    let HostActivationOutcome::Failed(reason) = outcome else {
        panic!("a command that hangs is a failed hand-off, got {outcome:?}");
    };
    assert!(reason.as_str().contains("150ms"), "{reason}");
}

#[tokio::test]
async fn output_past_the_bound_is_dropped_rather_than_read() {
    let script = HostScript::new("cat > /dev/null; printf 'abcdefghij'");

    let outcome = activate(&script.adapter_with(Duration::from_secs(5), 4)).await;

    let HostActivationOutcome::Accepted(receipt) = outcome else {
        panic!("a clean exit is an accepted hand-off, got {outcome:?}");
    };
    assert_eq!(
        receipt.transport_ref().map(HostTransportRef::as_str),
        Some("abcd")
    );
}

/// Chatty is not the same as broken.
///
/// A megabyte past a 64-byte bound is far more than any pipe buffer
/// holds, so a capture that stopped reading at the bound would hand the
/// command `EPIPE`, kill it, and turn a wake-up that reached the host
/// into a failure that came back forever.
#[tokio::test]
async fn a_command_that_writes_far_past_the_bound_still_succeeds() {
    let script = HostScript::new(
        "cat > /dev/null; printf 'ref-64-bytes'; \
         dd if=/dev/zero bs=1024 count=1024 2>/dev/null | tr '\\0' 'x'",
    );

    let outcome = activate(&script.adapter_with(Duration::from_secs(20), 64)).await;

    let HostActivationOutcome::Accepted(receipt) = outcome else {
        panic!("a command that talks too much is still a command that answered, got {outcome:?}");
    };
    let reference = receipt
        .transport_ref()
        .map(HostTransportRef::as_str)
        .unwrap();
    assert!(reference.starts_with("ref-64-bytes"), "{reference}");
    assert_eq!(reference.len(), 64, "{reference}");
}

/// An orphan holding the pipe must not hold the engine with it.
///
/// A wrapper that backgrounds its work and returns leaves a child with
/// the inherited stdout still open, so the reader never sees EOF. The
/// same deadline that bounds the wait bounds the drain, and what was
/// read by then is what the receipt carries.
#[tokio::test]
async fn a_command_that_leaves_an_orphan_holding_the_pipe_still_returns() {
    let script = HostScript::new(
        "cat > /dev/null; printf 'woke-and-left'; sleep 30 & \
         echo $! > \"$0/orphan.pid\"; exit 0",
    );
    let adapter = script.adapter_with(Duration::from_millis(600), 65_536);

    let started = std::time::Instant::now();
    let outcome = activate(&adapter).await;
    let waited = started.elapsed();

    let HostActivationOutcome::Accepted(receipt) = outcome else {
        panic!(
            "the command exited clean; an orphan of its own is not its failure, got {outcome:?}"
        );
    };
    assert!(
        waited < Duration::from_secs(5),
        "the orphan held the call for {waited:?}"
    );
    assert_eq!(
        receipt.transport_ref().map(HostTransportRef::as_str),
        Some("woke-and-left")
    );
    let pid = script.read("orphan.pid").trim().to_owned();
    assert!(!pid.is_empty(), "the script never recorded its child");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "the orphan outlived the call as process {pid}"
    );
}

/// A timeout has to reach the turn, not just the wrapper that started it.
#[tokio::test]
async fn a_timeout_takes_down_what_the_command_started() {
    let script = HostScript::new(
        "cat > /dev/null; sleep 120 & \
         echo $! > \"$0/grandchild.pid\"; wait",
    );

    let outcome = activate(&script.adapter_with(Duration::from_millis(400), 65_536)).await;

    assert!(
        matches!(outcome, HostActivationOutcome::Failed(_)),
        "{outcome:?}"
    );
    let pid = script.read("grandchild.pid").trim().to_owned();
    assert!(!pid.is_empty(), "the script never recorded its child");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "the host turn outlived the timeout as process {pid}"
    );
}

#[tokio::test]
async fn the_reference_keeps_the_first_bytes_of_a_long_answer() {
    let script = HostScript::new("cat > /dev/null; printf 'r%.0s' $(seq 1 400)");

    let outcome = activate(&script.adapter()).await;

    let HostActivationOutcome::Accepted(receipt) = outcome else {
        panic!("a clean exit is an accepted hand-off, got {outcome:?}");
    };
    assert_eq!(
        receipt.transport_ref().map(|value| value.as_str().len()),
        Some(256)
    );
}

#[tokio::test]
async fn the_envelope_arrives_on_standard_input_and_never_as_arguments() {
    let script = HostScript::new(
        "cat > \"$0/stdin.json\"; printf '%s' \"$*\" \
         > \"$0/args.txt\"",
    );

    let outcome = activate(&script.adapter()).await;

    assert!(outcome.is_accepted(), "{outcome:?}");
    let received: serde_json::Value = serde_json::from_str(&script.read("stdin.json")).unwrap();
    assert_eq!(
        received["delivery_id"],
        serde_json::json!(record().id().as_str())
    );
    assert_eq!(received["ceremony_id"], serde_json::json!("c-1"));
    assert_eq!(script.read("args.txt"), "--one");
}

#[tokio::test]
async fn the_child_is_told_the_destination_and_nothing_else() {
    std::env::set_var("MADE_ACTIVATION_LEAK_CANARY", "must-not-cross");
    let script = HostScript::new("cat > /dev/null; env | sort > \"$0/env.txt\"");

    let outcome = activate(&script.adapter()).await;

    assert!(outcome.is_accepted(), "{outcome:?}");
    let environment = script.read("env.txt");
    let names: Vec<&str> = environment
        .lines()
        .filter_map(|line| line.split('=').next())
        .filter(|name| !name.is_empty() && *name != "PWD" && *name != "SHLVL" && *name != "_")
        .collect();
    assert_eq!(
        names,
        vec![DELIVERY_ID_VAR, DESTINATION_VAR, HOST_KIND_VAR],
        "{environment}"
    );
    assert!(
        environment.contains(&format!("{DESTINATION_VAR}=session-42")),
        "{environment}"
    );
    assert!(
        environment.contains(&format!("{HOST_KIND_VAR}=claude-code")),
        "{environment}"
    );
    assert!(!environment.contains("must-not-cross"), "{environment}");
}

#[tokio::test]
async fn a_command_that_cannot_be_started_is_a_failure_not_a_panic() {
    let directory = tempfile::tempdir().unwrap();
    let adapter = CommandHostActivation::new(
        HostActivationCommand::new(a_written_script(&directory), []).unwrap(),
        Duration::from_secs(5),
        65_536,
    );
    drop(directory);

    let outcome = activate(&adapter).await;

    assert!(
        matches!(outcome, HostActivationOutcome::Failed(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_name_that_resolves_nowhere_is_refused_at_configuration_time() {
    assert_eq!(
        HostActivationCommand::parse("made-no-such-host-command", COMMAND_ENV),
        Err(HostActivationConfigError::ExecutableUnavailable {
            command: "made-no-such-host-command".to_owned()
        })
    );
}

#[test]
fn a_directory_is_not_a_command() {
    let directory = tempfile::tempdir().unwrap();
    let raw = directory.path().display().to_string();
    assert!(matches!(
        HostActivationCommand::parse(&raw, COMMAND_ENV),
        Err(HostActivationConfigError::ExecutableNotFile { .. })
    ));
}

#[test]
fn a_resolved_command_keeps_its_arguments_in_order() {
    let directory = tempfile::tempdir().unwrap();
    let raw = format!(
        "{} --resume --print",
        a_written_script(&directory).display()
    );
    let command = HostActivationCommand::parse(&raw, COMMAND_ENV).unwrap();
    assert_eq!(command.args(), ["--resume", "--print"]);
    assert!(command.executable().is_absolute());
}
