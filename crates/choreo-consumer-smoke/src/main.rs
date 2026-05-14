//! `choreo-consumer-smoke` binary.
//!
//! A CLI a consumer can point at a live choreographer cluster to
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

use choreo_consumer_smoke::outcome::AssertionStatus;
use choreo_consumer_smoke::{run_chain_1, run_chain_2, ChainOutcome, Harness, HarnessConfig};
use clap::{Parser, ValueEnum};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(
    name = "choreo-consumer-smoke",
    about = "Drive the Choreographer's public surface as a generic consumer would."
)]
struct Args {
    #[arg(
        long,
        env = "CHOREOGRAPHER_ENDPOINT",
        default_value = "http://localhost:50055"
    )]
    endpoint: String,
    #[arg(long, env = "CHOREO_NATS_URL")]
    nats_url: Option<String>,
    #[arg(long, default_value = "triage")]
    specialty: String,
    #[arg(long, default_value = "consumer-smoke-report-v1")]
    contract_id: String,
    #[arg(long, value_enum, default_value_t = ChainSelector::All)]
    chain: ChainSelector,
    #[arg(long, default_value_t = 30)]
    connect_budget_secs: u64,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
enum ChainSelector {
    One,
    Two,
    All,
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
            eprintln!("error: could not connect to choreographer: {err:#}");
            return 2;
        }
    };

    let mut outcomes: Vec<ChainOutcome> = Vec::new();
    if args.chain == ChainSelector::One || args.chain == ChainSelector::All {
        match run_chain_1(&mut harness, &cfg).await {
            Ok(o) => outcomes.push(o),
            Err(err) => {
                eprintln!("error: chain1 failed to run: {err:#}");
                return 2;
            }
        }
    }
    if args.chain == ChainSelector::Two || args.chain == ChainSelector::All {
        match run_chain_2(&mut harness, &cfg).await {
            Ok(o) => outcomes.push(o),
            Err(err) => {
                eprintln!("error: chain2 failed to run: {err:#}");
                return 2;
            }
        }
    }

    print_table(&outcomes);
    print_summary(&outcomes);

    i32::from(outcomes.iter().any(|o| o.failed_count() > 0))
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
        let args = Args::try_parse_from(["choreo-consumer-smoke"]).unwrap();
        assert_eq!(args.endpoint, "http://localhost:50055");
        assert_eq!(args.specialty, "triage");
        assert_eq!(args.contract_id, "consumer-smoke-report-v1");
        assert_eq!(args.chain, ChainSelector::All);
        assert_eq!(args.connect_budget_secs, 30);
        assert!(args.nats_url.is_none());
    }

    #[test]
    fn args_parse_chain_two() {
        let args = Args::try_parse_from(["choreo-consumer-smoke", "--chain", "two"]).unwrap();
        assert_eq!(args.chain, ChainSelector::Two);
    }
}
