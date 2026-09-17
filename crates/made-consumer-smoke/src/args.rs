use clap::Parser;

use crate::{ChainSelector, ProviderKind};

#[derive(Parser, Debug)]
#[command(
    name = "made-consumer-smoke",
    about = "Drive the MADE's public surface as a generic consumer would."
)]
pub(super) struct Args {
    #[arg(long, env = "MADE_ENDPOINT", default_value = "http://localhost:50055")]
    pub(super) endpoint: String,
    #[arg(long, env = "MADE_NATS_URL")]
    pub(super) nats_url: Option<String>,
    #[arg(long, default_value = "triage")]
    pub(super) specialty: String,
    #[arg(long, default_value = "consumer-smoke-report-v1")]
    pub(super) contract_id: String,
    #[arg(long, value_enum, default_value_t = ChainSelector::All)]
    pub(super) chain: ChainSelector,
    #[arg(long, value_enum, default_value_t = ProviderKind::Openai, env = "CONSUMER_SMOKE_PROVIDER_KIND")]
    pub(super) provider_kind: ProviderKind,
    #[arg(long, env = "CONSUMER_SMOKE_PROVIDER_ENDPOINT")]
    pub(super) provider_endpoint: Option<String>,
    #[arg(
        long,
        env = "CONSUMER_SMOKE_PROVIDER_MODEL",
        default_value = "stub-report-v1"
    )]
    pub(super) provider_model: String,
    #[arg(long, env = "CONSUMER_SMOKE_POSITIVE_SPECIALTY")]
    pub(super) positive_specialty: Option<String>,
    #[arg(long, default_value_t = 30)]
    pub(super) connect_budget_secs: u64,
}
