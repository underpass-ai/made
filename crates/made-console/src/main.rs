use clap::Parser;

use args::Args;
use artifact_command::ArtifactCommand;
use authorization_command::AuthorizationCommand;
use authorization_scope_args::AuthorizationScopeArgs;
use authorization_scope_kind::AuthorizationScopeKind;
use budget_command::BudgetCommand;
use command::Command;
use output_format::OutputFormat;
use receipt_command::ReceiptCommand;

mod args;
mod artifact_command;
mod authorization_command;
mod authorization_execute;
mod authorization_render;
mod authorization_scope_args;
mod authorization_scope_kind;
mod budget_command;
mod command;
mod execute;
mod lifecycle_filter_arg;
mod output_format;
mod receipt_command;
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
