use made_core::{error::DomainError, value_objects::CeremonyId};

use super::{CeremonyReportIds, ReportTitle};

/// What to report on, and what to call the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateCeremonyReportInput {
    ceremony_ids: CeremonyReportIds,
    /// The heading. Absent takes the default one; what counts as
    /// present is [`ReportTitle`]'s to say, so the two arms cannot
    /// disagree about a padded string.
    title: Option<ReportTitle>,
}

impl GenerateCeremonyReportInput {
    pub fn new(
        ceremony_ids: Vec<CeremonyId>,
        title: Option<ReportTitle>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            ceremony_ids: CeremonyReportIds::new(ceremony_ids)?,
            title,
        })
    }

    #[must_use]
    pub fn from_ids(ceremony_ids: CeremonyReportIds, title: Option<ReportTitle>) -> Self {
        Self {
            ceremony_ids,
            title,
        }
    }

    /// The sessions to report on, in the order the caller asked for
    /// them. Order is the caller's; the report keeps it.
    #[must_use]
    pub fn ceremony_ids(&self) -> &[CeremonyId] {
        self.ceremony_ids.as_slice()
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_ref().map(ReportTitle::as_str)
    }
}
