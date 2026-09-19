use clap::Parser;

use crate::{Command, OutputFormat};

/// Operate MADE through its public gRPC API.
#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Args {
    /// Public MADE gRPC endpoint.
    #[arg(long, env = "MADE_ENDPOINT", default_value = "http://127.0.0.1:50055")]
    pub endpoint: String,
    /// Output format for metadata and events.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub output: OutputFormat,
    #[command(subcommand)]
    pub command: Command,
}
