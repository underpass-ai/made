use serde::{Deserialize, Serialize};

/// One reason this protocol cannot hand off yet, said to whoever asked.
///
/// A sentence rather than a code, because the answer is read by the
/// person deciding whether to hand off and the list is what they act
/// on. The typed refusals still exist where the decision is made; this
/// is the report that stops them being discovered by trial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProtocolSuccessionBlocker(String);

impl ProtocolSuccessionBlocker {
    #[must_use]
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
