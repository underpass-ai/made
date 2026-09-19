use std::path::PathBuf;

use clap::Subcommand;

use crate::{
    lifecycle_filter_arg::LifecycleFilterArg, ArtifactCommand, BudgetCommand, ReceiptCommand,
};

/// Public operator reads, exports and authorized actions.
#[derive(Debug, Subcommand)]
pub enum Command {
    Get {
        ceremony_id: String,
    },
    List {
        /// Opaque cursor returned by the preceding page.
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: u32,
        /// Literal ceremony id prefix; `%` and `_` have no wildcard meaning.
        #[arg(long)]
        id_prefix: Option<String>,
        #[arg(long, value_enum)]
        lifecycle: Option<LifecycleFilterArg>,
    },
    Tree {
        ceremony_id: String,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1000))]
        max_nodes: u32,
    },
    Watch {
        ceremony_id: String,
        #[arg(long)]
        cursor_file: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        after_sequence: u64,
        #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..=1000))]
        limit: u32,
        #[arg(long, default_value_t = 1000)]
        wait_ms: u32,
        #[arg(long)]
        follow: bool,
    },
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommand,
    },
    Budget {
        #[command(subcommand)]
        command: BudgetCommand,
    },
    Receipt {
        #[command(subcommand)]
        command: ReceiptCommand,
    },
    Report {
        #[arg(required = true, num_args = 1..)]
        ceremony_ids: Vec<String>,
        #[arg(long, default_value = "Ceremony report")]
        title: String,
        #[arg(long)]
        destination: PathBuf,
    },
    Pause {
        ceremony_id: String,
        #[arg(long)]
        actor_id: String,
        #[arg(long)]
        actor_kind: String,
        #[arg(long)]
        reason: String,
    },
    Resume {
        ceremony_id: String,
        #[arg(long)]
        actor_id: String,
        #[arg(long)]
        actor_kind: String,
    },
    Cancel {
        ceremony_id: String,
        #[arg(long)]
        actor_id: String,
        #[arg(long)]
        actor_kind: String,
        #[arg(long)]
        reason: String,
    },
    EnforceDeadlines {
        ceremony_id: String,
    },
    Approve {
        ceremony_id: String,
        #[arg(long)]
        guard: String,
        #[arg(long)]
        role_id: String,
        #[arg(long)]
        role_kind: String,
    },
}
