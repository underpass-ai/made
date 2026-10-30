use serde::{Deserialize, Serialize};

/// The caller's explanation for a causal link between protocol records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolReasonRationale(String);

impl ProtocolReasonRationale {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}
