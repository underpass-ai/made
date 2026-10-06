use serde::{Deserialize, Serialize};

/// The head as the file spells it: a version, a hex digest and a count.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HeadWire {
    pub(super) version: u64,
    pub(super) hash: String,
    pub(super) record_count: u64,
}
