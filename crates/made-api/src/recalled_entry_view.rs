use serde::{Deserialize, Serialize};

/// One thing an earlier session left behind, as a consumer sees it.
///
/// Plain data, like everything else here: the summary is what was
/// decided or seen, and the session and the moment are what let a
/// consumer attribute it rather than take it on trust.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecalledEntryView {
    pub entry_id: String,
    /// `decision`, `observation`, `constraint` or `outcome`.
    pub kind: String,
    pub summary: String,
    /// The session that said it.
    pub from_ceremony_id: String,
    pub observed_at_millis: i64,
}
