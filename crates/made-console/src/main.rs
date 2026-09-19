use clap::Parser;

use args::Args;
use artifact_command::ArtifactCommand;
use budget_command::BudgetCommand;
use command::Command;
use output_format::OutputFormat;

mod args;
mod artifact_command;
mod budget_command;
mod command;
mod execute;
mod output_format;
mod render;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() {
    if let Err(error) = execute::run(Args::parse()).await {
        eprintln!("made-console: {error}");
        std::process::exit(i32::from(error.exit_code()));
    }
}
