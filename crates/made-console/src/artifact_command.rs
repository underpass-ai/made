use std::path::PathBuf;

use clap::Subcommand;

/// Bounded artifact metadata and export operations.
#[derive(Debug, Subcommand)]
pub enum ArtifactCommand {
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
    },
    Export {
        artifact_id: String,
        destination: PathBuf,
        #[arg(long)]
        force: bool,
    },
}
