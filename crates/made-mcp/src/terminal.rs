//! The person's side of a terminal command: somewhere to show what is
//! about to be recorded and to ask whether it should be.
//!
//! `approve-guard` and `grant` both run here and nowhere else. The
//! channel is the point: everything on the MCP session was written by
//! the agent, so a decision the agent must not be able to take is put
//! to a person at a terminal the agent's session cannot write to.

mod stdio_terminal;

pub use stdio_terminal::StdioTerminal;

/// Somewhere to show what is about to be recorded and to ask whether
/// it should be.
///
/// A trait so a command can be proved with a scripted terminal; the
/// one the binary uses reads the real stdin and refuses to run when
/// that is not a terminal.
pub trait Terminal {
    /// Whether a person is at the other end. Piped input is not.
    fn is_interactive(&self) -> bool;

    /// Show one line of what is being decided.
    fn show(&mut self, line: &str);

    /// Ask, and return whether the person said yes.
    fn confirm(&mut self, prompt: &str) -> Result<bool, String>;
}
