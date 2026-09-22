//! Killing what a spawned command spawned in turn.
//!
//! A wrapper script is the normal shape of an operator's command: the
//! thing the engine starts is a shell, and the thing that does the work
//! is its child. Signalling only the process this crate spawned leaves
//! that child running — unsupervised, and on a metered host still being
//! paid for. Putting the command in its own process group makes the
//! whole tree addressable by one signal.

#[cfg(unix)]
use std::path::Path;

use tokio::process::Command;

/// Start the command as the leader of a new process group.
#[cfg(unix)]
pub(crate) fn lead_its_own_group(command: &mut Command) {
    command.process_group(0);
}

/// Nothing to do where process groups are not a concept.
#[cfg(not(unix))]
pub(crate) fn lead_its_own_group(_command: &mut Command) {}

/// Signal the whole group led by `pid`. Answers whether it worked.
///
/// `kill(2)` is reached through `/bin/kill` rather than a C binding
/// because this crate carries no `libc` dependency and is not about to
/// acquire one to send one signal.
#[cfg(unix)]
pub(crate) fn kill_group(pid: u32) -> bool {
    let Some(kill_binary) = ["/bin/kill", "/usr/bin/kill"]
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
    else {
        return false;
    };
    std::process::Command::new(kill_binary)
        .arg("-KILL")
        .arg("--")
        .arg(format!("-{pid}"))
        .status()
        .is_ok_and(|status| status.success())
}

/// Where groups do not exist, the caller falls back to the child alone.
#[cfg(not(unix))]
pub(crate) fn kill_group(_pid: u32) -> bool {
    false
}
