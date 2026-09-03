use serde::{Deserialize, Serialize};

/// Explanation recorded alongside an evidence-support decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SupportRationale(String);

impl SupportRationale {
    #[must_use]
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into().trim().to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
