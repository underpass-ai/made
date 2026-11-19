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

use std::process::{Output, Stdio};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const LABEL: &str = "made-test-container=1";
const STARTUP_TIMEOUT: Duration = Duration::from_mins(3);

fn docker_binary() -> String {
    std::env::var("MADE_TEST_DOCKER").unwrap_or_else(|_| "docker".to_owned())
}

/// Which output stream carries the readiness line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

/// What a container is started with.
#[derive(Debug, Clone)]
pub struct ContainerRequest {
    image: String,
    env: Vec<(String, String)>,
    cmd: Vec<String>,
    ports: Vec<u16>,
    mounts: Vec<String>,
    ready_line: Option<(LogStream, String)>,
}

impl ContainerRequest {
    /// `image` is a full reference, `<name>:<tag>`.
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            env: Vec::new(),
            cmd: Vec::new(),
            ports: Vec::new(),
            mounts: Vec::new(),
            ready_line: None,
        }
    }

    #[must_use]
    pub fn with_env_var(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    #[must_use]
    pub fn with_cmd<I, S>(mut self, cmd: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cmd = cmd.into_iter().map(Into::into).collect();
        self
    }

    /// Publish a container TCP port on a free `127.0.0.1` port.
    #[must_use]
    pub fn with_exposed_port(mut self, port: u16) -> Self {
        self.ports.push(port);
        self
    }

    #[must_use]
    pub fn with_bind_mount(
        mut self,
        host: impl AsRef<std::path::Path>,
        target: &str,
        read_only: bool,
    ) -> Self {
        let mode = if read_only { ",readonly" } else { "" };
        self.mounts.push(format!(
            "type=bind,source={},target={target}{mode}",
            host.as_ref().display()
        ));
        self
    }

    /// Start only counts once this text appears on `stream`.
    #[must_use]
    pub fn with_ready_line(mut self, stream: LogStream, text: impl Into<String>) -> Self {
        self.ready_line = Some((stream, text.into()));
        self
    }

    /// Run the container detached and wait for its readiness line.
    ///
    /// The image is pulled when it is not present. A container that
    /// exits, or never prints the line, is an error carrying its logs.
    pub async fn start(self) -> Result<Container, String> {
        let mut args: Vec<String> = vec![
            "run".into(),
            "--detach".into(),
            "--label".into(),
            LABEL.into(),
        ];
        for (name, value) in &self.env {
            args.push("--env".into());
            args.push(format!("{name}={value}"));
        }
        for port in &self.ports {
            args.push("--publish".into());
            args.push(format!("127.0.0.1::{port}/tcp"));
        }
        for mount in &self.mounts {
            args.push("--mount".into());
            args.push(mount.clone());
        }
        args.push(self.image.clone());
        args.extend(self.cmd.iter().cloned());

        let output = docker(&args).await?;
        let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if id.is_empty() {
            return Err(format!(
                "`docker run {}` printed no container id",
                self.image
            ));
        }
        let container = Container { id };
        if let Some((stream, text)) = &self.ready_line {
            container.wait_for_line(*stream, text).await?;
        }
        Ok(container)
    }
}

/// A running container, removed when dropped.
#[derive(Debug)]
pub struct Container {
    id: String,
}

impl Container {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The `127.0.0.1` port a published container port landed on.
    pub async fn host_port(&self, port: u16) -> Result<u16, String> {
        let output = docker(&["port".into(), self.id.clone(), format!("{port}/tcp")]).await?;
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .filter_map(|line| line.trim().rsplit_once(':'))
            .find_map(|(_, port)| port.parse().ok())
            .ok_or_else(|| format!("container port {port} is not published: {text}"))
    }

    /// Run `command` inside the container; a non-zero exit is an error.
    pub async fn exec<I, S>(&self, command: I) -> Result<Output, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = vec!["exec".to_owned(), self.id.clone()];
        args.extend(command.into_iter().map(Into::into));
        docker(&args).await
    }

    async fn wait_for_line(&self, stream: LogStream, text: &str) -> Result<(), String> {
        // Only the watched stream is piped: an unread pipe fills up and
        // stalls `docker logs` on a chatty container.
        let (stdout, stderr) = match stream {
            LogStream::Stdout => (Stdio::piped(), Stdio::null()),
            LogStream::Stderr => (Stdio::null(), Stdio::piped()),
        };
        let mut child = Command::new(docker_binary())
            .args(["logs", "--follow", &self.id])
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| format!("cannot follow container logs: {error}"))?;
        let pipe: Box<dyn tokio::io::AsyncRead + Unpin + Send> = match stream {
            LogStream::Stdout => Box::new(child.stdout.take().expect("piped stdout")),
            LogStream::Stderr => Box::new(child.stderr.take().expect("piped stderr")),
        };
        let mut lines = BufReader::new(pipe).lines();
        let found = tokio::time::timeout(STARTUP_TIMEOUT, async {
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains(text) {
                    return true;
                }
            }
            false
        })
        .await;
        let _ = child.kill().await;
        match found {
            Ok(true) => Ok(()),
            Ok(false) => Err(format!(
                "container exited before printing `{text}`:\n{}",
                self.logs().await
            )),
            Err(_) => Err(format!(
                "container did not print `{text}` within {STARTUP_TIMEOUT:?}:\n{}",
                self.logs().await
            )),
        }
    }

    async fn logs(&self) -> String {
        match Command::new(docker_binary())
            .args(["logs", &self.id])
            .output()
            .await
        {
            Ok(output) => format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(error) => format!("(logs unavailable: {error})"),
        }
    }
}

impl Drop for Container {
    fn drop(&mut self) {
        let _ = std::process::Command::new(docker_binary())
            .args(["rm", "--force", "--volumes", &self.id])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
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
