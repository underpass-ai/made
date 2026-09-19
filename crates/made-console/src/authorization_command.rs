use clap::Subcommand;

use crate::AuthorizationScopeArgs;

#[derive(Debug, Subcommand)]
pub enum AuthorizationCommand {
    Policy,
    Issue {
        grant_id: String,
        grantee_id: String,
        #[arg(long, required = true, value_delimiter = ',')]
        actions: Vec<String>,
        #[command(flatten)]
        scope: AuthorizationScopeArgs,
        #[arg(long)]
        valid_from: String,
        #[arg(long)]
        valid_until: Option<String>,
        #[arg(long, default_value_t = 0)]
        delegation_depth: u32,
        #[arg(long)]
        parent_grant_id: Option<String>,
    },
    Revoke {
        grant_id: String,
        #[arg(long)]
        reason: String,
    },
    Decisions {
        #[arg(long)]
        after: Option<String>,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=500))]
        limit: u32,
    },
}
