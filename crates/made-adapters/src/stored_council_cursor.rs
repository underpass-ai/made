use made_core::value_objects::{CouncilJournalLease, CouncilJournalPosition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct StoredCouncilCursor {
    pub(crate) position: Option<CouncilJournalPosition>,
    pub(crate) lease: Option<CouncilJournalLease>,
}
