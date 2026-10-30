use made_core::entities::{AuditRecord, ProtocolAgentActivity};

use super::{ProtocolAgentSnapshot, ProtocolProgressEnd};

/// One sealed record or the successful end of a bounded progress stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolProgressFrame {
    Record(Box<AuditRecord>),
    AgentSnapshot(ProtocolAgentSnapshot),
    AgentActivity(Box<ProtocolAgentActivity>),
    End(ProtocolProgressEnd),
}
