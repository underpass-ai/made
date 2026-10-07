use std::io::{self, BufRead, IsTerminal, Write};

use super::Terminal;

/// The process's own stdin and stdout, when both are terminals.
#[derive(Debug, Default)]
pub struct StdioTerminal;

impl Terminal for StdioTerminal {
    fn is_interactive(&self) -> bool {
        io::stdin().is_terminal() && io::stdout().is_terminal()
    }

    fn show(&mut self, line: &str) {
        println!("{line}");
    }

    fn confirm(&mut self, prompt: &str) -> Result<bool, String> {
        print!("{prompt} [y/N] ");
        io::stdout()
            .flush()
            .map_err(|error| format!("could not write the prompt: {error}"))?;
        let mut answer = String::new();
        io::stdin()
            .lock()
            .read_line(&mut answer)
            .map_err(|error| format!("could not read the answer: {error}"))?;
        let answer = answer.trim().to_ascii_lowercase();
        Ok(answer == "y" || answer == "yes")
    }
}
