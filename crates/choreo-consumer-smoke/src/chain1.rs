//! Chain 1 placeholder — the live implementation lands in the
//! follow-up commit by the agent. The signature is fixed so the
//! binary + tests can wire against it from the start.

use anyhow::Result;

use crate::{ChainOutcome, Harness, HarnessConfig};

/// To be implemented in the chain-1 commit. See plan in
/// `docs/backlog.md` Epic 12.
pub async fn run_chain_1(_h: &mut Harness, _cfg: &HarnessConfig) -> Result<ChainOutcome> {
    anyhow::bail!("run_chain_1 is not implemented yet (Epic 12 follow-up commit)")
}
