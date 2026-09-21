use std::path::{Path, PathBuf};

use super::HostActivationConfigError;

/// The one command an operator configured, already resolved to a file.
///
/// Resolved once, at startup, and kept as an absolute path: the
/// alternative is looking the name up again at every delivery, which
/// makes what runs depend on the PATH of whoever happened to start the
/// process. Nothing here ever comes from an envelope — the arguments
/// are the operator's, fixed before the first host is woken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostActivationCommand {
    executable: PathBuf,
    args: Vec<String>,
}

impl HostActivationCommand {
    /// Read `executable arg...` and resolve the executable.
    ///
    /// # Errors
    ///
    /// Returns the configuration failure when the value names nothing,
    /// when the name resolves nowhere, or when it resolves to something
    /// that is not a file.
    pub fn parse(raw: &str, variable: &'static str) -> Result<Self, HostActivationConfigError> {
        let mut words = raw.split_whitespace();
        let Some(name) = words.next() else {
            return Err(HostActivationConfigError::EmptyCommand { variable });
        };
        Ok(Self {
            executable: resolve(name)?,
            args: words.map(ToOwned::to_owned).collect(),
        })
    }

    /// Build a resolved command directly, for a caller that already has one.
    ///
    /// # Errors
    ///
    /// Returns the configuration failure when the path does not resolve
    /// to a file.
    pub fn new(
        executable: impl AsRef<Path>,
        args: impl IntoIterator<Item = String>,
    ) -> Result<Self, HostActivationConfigError> {
        Ok(Self {
            executable: canonical_file(executable.as_ref())?,
            args: args.into_iter().collect(),
        })
    }

    #[must_use]
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

fn resolve(name: &str) -> Result<PathBuf, HostActivationConfigError> {
    if name.contains(std::path::MAIN_SEPARATOR) {
        return canonical_file(Path::new(name));
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| HostActivationConfigError::ExecutableUnavailable {
            command: name.to_owned(),
        })
        .and_then(|candidate| canonical_file(&candidate))
}

fn canonical_file(candidate: &Path) -> Result<PathBuf, HostActivationConfigError> {
    let resolved =
        candidate
            .canonicalize()
            .map_err(|_| HostActivationConfigError::ExecutableUnavailable {
                command: candidate.display().to_string(),
            })?;
    if !resolved.is_file() {
        return Err(HostActivationConfigError::ExecutableNotFile { path: resolved });
    }
    Ok(resolved)
}
