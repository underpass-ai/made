use std::io;

use thiserror::Error;

use super::LocalExecutionMode;

#[derive(Debug, Error)]
pub enum LocalExecutionError {
    #[error("local execution mode {mode:?} is unsupported: isolated Linux runtime is unavailable")]
    UnsupportedMode { mode: LocalExecutionMode },
    #[error("local execution filesystem root is unavailable")]
    FilesystemRootUnavailable,
    #[error("local execution filesystem root must be a directory")]
    FilesystemRootNotDirectory,
    #[error("local execution timeout must be non-zero")]
    InvalidTimeout,
    #[error("local execution output limit must be non-zero")]
    InvalidOutputLimit,
    #[error("local execution executable is unavailable")]
    ExecutableUnavailable,
    #[error("local execution executable must remain below the configured filesystem root")]
    ExecutableOutsideRoot,
    #[error("local execution executable is not a file")]
    ExecutableNotFile,
    #[error("local execution output pipes are unavailable")]
    OutputPipeUnavailable,
    #[error("local execution process could not be spawned")]
    Spawn { source: io::Error },
    #[error("local execution process could not be awaited")]
    Wait,
    #[error("local execution output capture failed")]
    OutputCapture,
}
