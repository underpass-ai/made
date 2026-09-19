use crate::stored_council_cursor::StoredCouncilCursor;
use made_core::entities::CouncilJournalRecord;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub(super) struct CouncilJournalState {
    pub(super) records: Vec<CouncilJournalRecord>,
    pub(super) publications: BTreeMap<String, CouncilJournalRecord>,
    pub(super) cursors: BTreeMap<String, StoredCouncilCursor>,
}
