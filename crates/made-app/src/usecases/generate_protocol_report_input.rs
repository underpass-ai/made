use made_core::{error::DomainError, value_objects::ProtocolId};

use super::{ProtocolReportIds, ReportTitle};

/// What to report on, and what to call the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateProtocolReportInput {
    protocol_ids: ProtocolReportIds,
    /// The heading. Absent takes the default one; what counts as
    /// present is [`ReportTitle`]'s to say, so the two arms cannot
    /// disagree about a padded string.
    title: Option<ReportTitle>,
}

impl GenerateProtocolReportInput {
    pub fn new(
        protocol_ids: Vec<ProtocolId>,
        title: Option<ReportTitle>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            protocol_ids: ProtocolReportIds::new(protocol_ids)?,
            title,
        })
    }

    #[must_use]
    pub fn from_ids(protocol_ids: ProtocolReportIds, title: Option<ReportTitle>) -> Self {
        Self {
            protocol_ids,
            title,
        }
    }

    /// The sessions to report on, in the order the caller asked for
    /// them. Order is the caller's; the report keeps it.
    #[must_use]
    pub fn protocol_ids(&self) -> &[ProtocolId] {
        self.protocol_ids.as_slice()
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_ref().map(ReportTitle::as_str)
    }
}
