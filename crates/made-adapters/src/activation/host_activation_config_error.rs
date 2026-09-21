use std::fmt;
use std::path::PathBuf;

/// Why an operator's activation configuration could not be used.
///
/// A deployment that meant to wake hosts and cannot must say so at
/// startup rather than at the first delivery: a misspelled command
/// discovered hours later looks exactly like a host that never had
/// anything to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostActivationConfigError {
    /// The variable is set to whitespace: an intention without a command.
    EmptyCommand { variable: &'static str },
    /// Nothing on PATH, and nothing at that path, answers to the name.
    ExecutableUnavailable { command: String },
    /// The name resolves, but not to a file anything can run.
    ExecutableNotFile { path: PathBuf },
    /// A bound was configured as something that is not a positive number.
    InvalidBound {
        variable: &'static str,
        value: String,
    },
}

impl fmt::Display for HostActivationConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommand { variable } => {
                write!(formatter, "{variable} is set but names no command")
            }
            Self::ExecutableUnavailable { command } => write!(
                formatter,
                "the activation command `{command}` was not found on PATH or at that path"
            ),
            Self::ExecutableNotFile { path } => write!(
                formatter,
                "the activation command `{}` is not a file",
                path.display()
            ),
            Self::InvalidBound { variable, value } => write!(
                formatter,
                "{variable} must be a positive whole number, got `{value}`"
            ),
        }
    }
}

impl std::error::Error for HostActivationConfigError {}
