//! A throwaway Docker container for the container-backed test suites.
//!
//! Drives the `docker` CLI (`MADE_TEST_DOCKER` names another binary, such
//! as `podman`). No container client crate is linked: the suites only
//! need to run an image, wait for its readiness line, find a mapped port,
//! run a command inside it, and remove it again, and the CLI already
//! does all of that.
//!
//! Every container is labelled `made-test-container=1`, publishes its
//! ports on `127.0.0.1` only, and is removed with its anonymous volumes
//! when the handle drops, including when a test panics. A process killed
//! outright leaves the container behind; `docker rm -f $(docker ps -aq
//! --filter label=made-test-container=1)` cleans up.

mod container;
mod container_request;
mod log_stream;

use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::process::Command;

pub use container::Container;
pub use container_request::ContainerRequest;
pub use log_stream::LogStream;

const LABEL: &str = "made-test-container=1";
const STARTUP_TIMEOUT: Duration = Duration::from_mins(3);

fn docker_binary() -> String {
    std::env::var("MADE_TEST_DOCKER").unwrap_or_else(|_| "docker".to_owned())
}

async fn docker(args: &[String]) -> Result<Output, String> {
    let output = Command::new(docker_binary())
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| format!("cannot run {}: {error}", docker_binary()))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "`{} {}` failed with {}: {}",
            docker_binary(),
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
