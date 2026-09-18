use made_core::value_objects::{CouncilJournalLease, CouncilJournalPosition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct StoredCouncilCursor {
    pub(super) position: Option<CouncilJournalPosition>,
    pub(super) lease: Option<CouncilJournalLease>,
}
