use uuid::Uuid;

use crate::MadeClientError;

/// Namespace for one logical invocation and any reconstruction of its RPCs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestContext {
    namespace: String,
}

impl RequestContext {
    #[must_use]
    pub fn new() -> Self {
        Self {
            namespace: Uuid::new_v4().to_string(),
        }
    }

    pub fn from_id(id: impl Into<String>) -> Result<Self, MadeClientError> {
        let id = id.into();
        if id.trim().is_empty() || id.len() > 256 {
            return Err(MadeClientError::InvalidInvocationId);
        }
        Ok(Self { namespace: id })
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.namespace
    }
}

impl Default for RequestContext {
    fn default() -> Self {
        Self::new()
    }
}
