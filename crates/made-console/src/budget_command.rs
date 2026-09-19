use clap::Subcommand;

/// Durable budget balance and unresolved reservations.
#[derive(Debug, Subcommand)]
pub enum BudgetCommand {
    Report {
        ceremony_id: String,
    },
    Pending {
        #[arg(long, default_value = "")]
        after: String,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=1000))]
        limit: u32,
    },
}
