/// The person's side of `approve-guard`: somewhere to show what is about
/// to be recorded and to ask whether it should be.
///
/// A trait so the command can be proved with a scripted terminal; the
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
