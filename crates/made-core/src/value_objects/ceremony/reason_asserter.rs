use serde::{Deserialize, Serialize};

/// Actor category allowed to assert a ceremony reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonAsserter {
    TheEngine,
    ItsAuthor,
    AnySeat,
}
