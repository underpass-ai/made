use std::process::{Output, Stdio};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use super::{docker, docker_binary, LogStream, STARTUP_TIMEOUT};

/// A running container, removed when dropped.
#[derive(Debug)]
pub struct Container {
    id: String,
}

impl Container {
    pub(super) fn from_id(id: String) -> Self {
        Self { id }
    }

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

    pub(super) async fn wait_for_line(&self, stream: LogStream, text: &str) -> Result<(), String> {
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
