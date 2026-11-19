use super::{docker, Container, LogStream, LABEL};

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
        let container = Container::from_id(id);
        if let Some((stream, text)) = &self.ready_line {
            container.wait_for_line(*stream, text).await?;
        }
        Ok(container)
    }
}
