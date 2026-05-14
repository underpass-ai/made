//! Chain 2 placeholder — the live implementation lands in the
//! follow-up commit by the agent.

use anyhow::Result;

use crate::{ChainOutcome, Harness, HarnessConfig};

/// To be implemented in the chain-2 commit. See plan in
/// `docs/backlog.md` Epic 12.
pub async fn run_chain_2(_h: &mut Harness, _cfg: &HarnessConfig) -> Result<ChainOutcome> {
    anyhow::bail!("run_chain_2 is not implemented yet (Epic 12 follow-up commit)")
}
