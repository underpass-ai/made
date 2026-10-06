//! Where a human approval may come from.
//!
//! A human guard is a seat at the table that only a person may fill.
//! The engine records what it is told and does not vouch for who was
//! there — a declaration of `human` is a declaration. What a server can
//! decide is which channel it accepts that declaration on. The MCP
//! session is the agent's channel: everything on it was written by the
//! agent, the relayed decisions of a person included. A terminal the
//! person opens is theirs.
//!
//! In `terminal` mode the server refuses `made_approve_ceremony_guard`
//! altogether and points at `made-mcp approve-guard`, a command that
//! runs only at an interactive terminal, shows what is being approved
//! and asks. The record it seals is the same record; the difference is
//! that the agent cannot write it.

use serde_json::{json, Value};

/// Selects the channel: `host` (the default: the MCP session may record a
/// relayed human decision) or `terminal` (only the terminal command may).
pub const HUMAN_APPROVAL_SOURCE_ENV: &str = "MADE_HUMAN_APPROVAL_SOURCE";

/// The command a person runs, as discovery and refusals spell it.
pub const TERMINAL_APPROVAL_COMMAND: &str =
    "made-mcp approve-guard <store> --ceremony <id> --guard <name> --role <role> [--reason <text>]";

/// The plugin wrapper that fills in the store and the policy for them.
pub const TERMINAL_APPROVAL_SCRIPT: &str = "scripts/made-approve.sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HumanApprovalSource {
    /// The MCP session records relayed human decisions. The default of
    /// the bare binary, and what every gRPC deployment does.
    #[default]
    Host,
    /// Only the interactive terminal command records them. The default
    /// of the plugin launcher.
    Terminal,
}

impl HumanApprovalSource {
    /// Read the source from [`HUMAN_APPROVAL_SOURCE_ENV`]; unset or blank
    /// is `host`, and any other word is an error rather than a silent
    /// fallback to the channel the agent controls.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var(HUMAN_APPROVAL_SOURCE_ENV) {
            Ok(value) if !value.trim().is_empty() => Self::parse(&value),
            _ => Ok(Self::Host),
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "host" => Ok(Self::Host),
            "terminal" => Ok(Self::Terminal),
            other => Err(format!(
                "{HUMAN_APPROVAL_SOURCE_ENV} must be `host` or `terminal`; got `{other}`"
            )),
        }
    }

    pub fn from_name(name: &str) -> Result<Self, String> {
        Self::parse(name)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Terminal => "terminal",
        }
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Terminal)
    }

    /// The refusal the MCP tool answers with in terminal mode.
    #[must_use]
    pub fn refusal(self) -> String {
        format!(
            "human approvals are recorded from the person's own terminal on this server \
             ({HUMAN_APPROVAL_SOURCE_ENV}=terminal): ask the person to run \
             `{TERMINAL_APPROVAL_COMMAND}` (the plugin ships {TERMINAL_APPROVAL_SCRIPT}), \
             then read the instance again. An agent cannot record this decision."
        )
    }

    /// The discovery entry: which channel this server accepts, and what
    /// it does and does not prove.
    #[must_use]
    pub fn describe(self) -> Value {
        json!({
            "source": self.as_str(),
            "env": HUMAN_APPROVAL_SOURCE_ENV,
            "terminal_command": TERMINAL_APPROVAL_COMMAND,
            "plugin_script": TERMINAL_APPROVAL_SCRIPT,
            "proves": match self {
                Self::Host => "The approving actor kind is declared by the MCP caller. The sealed record says what the agent said; it does not establish that a person decided.",
                Self::Terminal => "An approval was confirmed at an interactive terminal the agent's MCP session cannot write to. It establishes the channel, not the identity of whoever sat at it; a process that can drive a pseudo-terminal can confirm one.",
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_channels_and_refuses_anything_else() {
        assert_eq!(
            HumanApprovalSource::parse(" Host ").unwrap(),
            HumanApprovalSource::Host
        );
        assert_eq!(
            HumanApprovalSource::parse("terminal").unwrap(),
            HumanApprovalSource::Terminal
        );
        let error = HumanApprovalSource::parse("agent").unwrap_err();
        assert!(error.contains(HUMAN_APPROVAL_SOURCE_ENV), "{error}");
        assert_eq!(HumanApprovalSource::default(), HumanApprovalSource::Host);
    }

    #[test]
    fn the_refusal_and_the_description_name_the_command() {
        let refusal = HumanApprovalSource::Terminal.refusal();
        assert!(refusal.contains("made-mcp approve-guard"), "{refusal}");
        assert!(refusal.contains(TERMINAL_APPROVAL_SCRIPT), "{refusal}");
        let description = HumanApprovalSource::Terminal.describe();
        assert_eq!(description["source"], "terminal");
        assert_eq!(description["terminal_command"], TERMINAL_APPROVAL_COMMAND);
        assert!(description["proves"]
            .as_str()
            .unwrap()
            .contains("pseudo-terminal"));
        assert_eq!(HumanApprovalSource::Host.describe()["source"], "host");
    }
}
