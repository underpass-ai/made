use made_core::entities::{AuditRecord, CeremonyAgentActivity};

use super::{CeremonyAgentSnapshot, CeremonyProgressEnd};

/// One sealed record or the successful end of a bounded progress stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyProgressFrame {
    Record(Box<AuditRecord>),
    AgentSnapshot(CeremonyAgentSnapshot),
    AgentActivity(Box<CeremonyAgentActivity>),
    End(CeremonyProgressEnd),
}
