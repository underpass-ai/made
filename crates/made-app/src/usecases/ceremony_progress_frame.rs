use made_core::entities::AuditRecord;

use super::CeremonyProgressEnd;

/// One sealed record or the successful end of a bounded progress stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeremonyProgressFrame {
    Record(Box<AuditRecord>),
    End(CeremonyProgressEnd),
}
