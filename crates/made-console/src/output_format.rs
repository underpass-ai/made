use clap::ValueEnum;

/// Stable machine JSON or readable indented output.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}
