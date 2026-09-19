use clap::Subcommand;

/// Immutable execution observations and bounded recovery inspection.
#[derive(Debug, Subcommand)]
pub enum ReceiptCommand {
    Get {
        operation_id: String,
    },
    Recovery {
        /// Opaque operation cursor returned by the preceding page.
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=500))]
        limit: u32,
    },
}
