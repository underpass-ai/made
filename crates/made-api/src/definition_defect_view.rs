use serde::{Deserialize, Serialize};

/// One defect found while analyzing a definition draft.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionDefectView {
    pub severity: String,
    pub locus: String,
    pub defect: String,
    pub blocking: bool,
}
