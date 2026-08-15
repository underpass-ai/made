//! `made-consumer-smoke` binary.
//!
//! A CLI a consumer can point at a live MADE cluster to
//! validate the public surface end-to-end. Runs Chain 1 and / or
//! Chain 2, prints a per-assertion table, and exits with:
//!
//! - `0` — every selected chain `passed()` (at least one assertion
//!   Passed and no Failed).
//! - `1` — at least one chain has a Failed assertion.
//! - `2` — infrastructure error (could not connect to the gRPC
//!   endpoint within the budget, for example).
//!
//! See `docs/operations/consumer-smoke.md` for the operational doc.

use std::time::Duration;

use clap::{Parser, ValueEnum};
use made_consumer_smoke::outcome::AssertionStatus;
use made_consumer_smoke::{
    run_chain_1, run_chain_2, run_positive_path, ChainOutcome, Harness, HarnessConfig,
    PositivePathConfig,
};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "made-consumer-smoke",
    about = "Drive the MADE's public surface as a generic consumer would."
)]
struct Args {
    #[arg(long, env = "MADE_ENDPOINT", default_value = "http://localhost:50055")]
    endpoint: String,
    #[arg(long, env = "MADE_NATS_URL")]
    nats_url: Option<String>,
    #[arg(long, default_value = "triage")]
    specialty: String,
    #[arg(long, default_value = "consumer-smoke-report-v1")]
    contract_id: String,
    #[arg(long, value_enum, default_value_t = ChainSelector::All)]
    chain: ChainSelector,
    #[arg(
        long,
        value_enum,
        default_value_t = ProviderKind::Openai,
        env = "CONSUMER_SMOKE_PROVIDER_KIND"
    )]
    provider_kind: ProviderKind,
    #[arg(long, env = "CONSUMER_SMOKE_PROVIDER_ENDPOINT")]
    provider_endpoint: Option<String>,
    #[arg(
        long,
        env = "CONSUMER_SMOKE_PROVIDER_MODEL",
        default_value = "stub-report-v1"
    )]
    provider_model: String,
    #[arg(long, env = "CONSUMER_SMOKE_POSITIVE_SPECIALTY")]
    positive_specialty: Option<String>,
    #[arg(long, default_value_t = 30)]
    connect_budget_secs: u64,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum ChainSelector {
    One,
    Two,
    #[value(name = "positive-path", alias = "positive", alias = "three")]
    PositivePath,
    All,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum ProviderKind {
    Openai,
    Vllm,
}

impl ProviderKind {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Vllm => "vllm",
        }
    }
}

#[tokio::main]
async fn main() {
    let exit = run().await;
    std::process::exit(exit);
}

async fn run() -> i32 {
    // `tracing-subscriber` honours `RUST_LOG`; fall back to `info` so
    // the CLI is useful out of the box.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let args = Args::parse();
    let cfg = HarnessConfig {
        endpoint: args.endpoint.clone(),
        nats_url: args.nats_url.clone(),
        specialty: args.specialty.clone(),
        contract_id: args.contract_id.clone(),
        connect_budget: Duration::from_secs(args.connect_budget_secs),
    };

    let mut harness = match Harness::connect(&cfg).await {
        Ok(h) => h,
        Err(err) => {
            eprintln!("error: could not connect to made: {err:#}");
            return 2;
        }
    };

    let mut outcomes: Vec<ChainOutcome> = Vec::new();
    match args.chain {
        ChainSelector::One => {
            if run_chain_one(&mut harness, &cfg, &mut outcomes)
                .await
                .is_err()
            {
                return 2;
            }
        }
        ChainSelector::Two => {
            if run_chain_two(&mut harness, &cfg, &mut outcomes)
                .await
                .is_err()
            {
                return 2;
            }
        }
        ChainSelector::All => {
            // Chain 2 registers the Report contract. Running it first
            // makes the default smoke usable against a fresh registry
            // before Chain 1 consumes the same contract in Warn mode.
            if run_chain_two(&mut harness, &cfg, &mut outcomes)
                .await
                .is_err()
            {
                return 2;
            }
            if run_chain_one(&mut harness, &cfg, &mut outcomes)
                .await
                .is_err()
            {
                return 2;
            }
        }
        ChainSelector::PositivePath => {
            let provider_kind = args.provider_kind.as_str().to_owned();
            let positive_cfg = PositivePathConfig {
                specialty: args
                    .positive_specialty
                    .clone()
                    .unwrap_or_else(|| format!("consumer-smoke-report-{provider_kind}")),
                agent_kind: provider_kind,
                agent_endpoint: args.provider_endpoint.clone(),
                agent_model: args.provider_model.clone(),
            };
            match run_positive_path(&mut harness, &cfg, &positive_cfg).await {
                Ok(o) => outcomes.push(o),
                Err(err) => {
                    eprintln!("error: positive-path failed to run: {err:#}");
                    return 2;
                }
            }
        }
    }

    print_table(&outcomes);
    print_summary(&outcomes);

    i32::from(outcomes.iter().any(|o| o.failed_count() > 0))
}

async fn run_chain_one(
    harness: &mut Harness,
    cfg: &HarnessConfig,
    outcomes: &mut Vec<ChainOutcome>,
) -> Result<(), ()> {
    match run_chain_1(harness, cfg).await {
        Ok(o) => {
            outcomes.push(o);
            Ok(())
        }
        Err(err) => {
            eprintln!("error: chain1 failed to run: {err:#}");
            Err(())
        }
    }
}

async fn run_chain_two(
    harness: &mut Harness,
    cfg: &HarnessConfig,
    outcomes: &mut Vec<ChainOutcome>,
) -> Result<(), ()> {
    match run_chain_2(harness, cfg).await {
        Ok(o) => {
            outcomes.push(o);
            Ok(())
        }
        Err(err) => {
            eprintln!("error: chain2 failed to run: {err:#}");
            Err(())
        }
    }
}

fn print_table(outcomes: &[ChainOutcome]) {
    // Hand-rolled 4-column table. `tabled` isn't in the workspace and
    // the per-line shape is regular enough that adding it for cosmetics
    // would be gold-plating.
    let col1 = outcomes
        .iter()
        .flat_map(|o| o.assertions.iter().map(|_| o.chain))
        .map(str::len)
        .max()
        .unwrap_or(6)
        .max(5);
    let col2 = outcomes
        .iter()
        .flat_map(|o| o.assertions.iter().map(|a| a.name))
        .map(str::len)
        .max()
        .unwrap_or(8)
        .max(9);

    for outcome in outcomes {
        for assertion in &outcome.assertions {
            let (verdict, detail) = match &assertion.status {
                AssertionStatus::Passed => ("PASS", String::new()),
                AssertionStatus::Skipped { reason } => ("SKIP", reason.clone()),
                AssertionStatus::Failed { detail } => ("FAIL", detail.clone()),
            };
            let elapsed_ms = duration_ms(assertion.duration);
            println!(
                "{chain:<col1$}  {name:<col2$}  {verdict}  {elapsed_ms:>4}ms  {detail}",
                chain = outcome.chain,
                name = assertion.name,
                col1 = col1,
                col2 = col2,
            );
        }
    }
}

fn print_summary(outcomes: &[ChainOutcome]) {
    println!();
    for outcome in outcomes {
        let label = if outcome.passed() { "PASS" } else { "FAIL" };
        let total = outcome.assertions.len();
        println!(
            "Summary: {chain} {label} ({p}/{t} passed, {s} skipped, {f} failed)",
            chain = outcome.chain,
            p = outcome.passed_count(),
            t = total,
            s = outcome.skipped_count(),
            f = outcome.failed_count(),
        );
    }
}

fn duration_ms(d: Duration) -> u128 {
    d.as_millis()
}

/// Re-exported so `clap` derives can be unit-tested if a future change
/// adds option validation logic.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parse_defaults() {
        let args = Args::try_parse_from(["made-consumer-smoke"]).unwrap();
        assert_eq!(args.endpoint, "http://localhost:50055");
        assert_eq!(args.specialty, "triage");
        assert_eq!(args.contract_id, "consumer-smoke-report-v1");
        assert_eq!(args.chain, ChainSelector::All);
        assert_eq!(args.connect_budget_secs, 30);
        assert!(args.nats_url.is_none());
    }

    #[test]
    fn args_parse_chain_two() {
        let args = Args::try_parse_from(["made-consumer-smoke", "--chain", "two"]).unwrap();
        assert_eq!(args.chain, ChainSelector::Two);
    }

    #[test]
    fn args_parse_positive_path_provider_options() {
        let args = Args::try_parse_from([
            "made-consumer-smoke",
            "--chain",
            "positive-path",
            "--provider-kind",
            "vllm",
            "--provider-endpoint",
            "http://stub-llm:8000",
            "--provider-model",
            "stub-report-vllm-v1",
            "--positive-specialty",
            "report-vllm",
        ])
        .unwrap();
        assert_eq!(args.chain, ChainSelector::PositivePath);
        assert_eq!(args.provider_kind, ProviderKind::Vllm);
        assert_eq!(
            args.provider_endpoint.as_deref(),
            Some("http://stub-llm:8000")
        );
        assert_eq!(args.provider_model, "stub-report-vllm-v1");
        assert_eq!(args.positive_specialty.as_deref(), Some("report-vllm"));
    }
}
