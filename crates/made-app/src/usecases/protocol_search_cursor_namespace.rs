use made_core::error::DomainError;

const MAX_COMPONENT_BYTES: usize = 256;

/// Stable deployment scope for protocol search cursors.
///
/// A store identifier and authorization policy identifier prevent a key
/// shared accidentally between installations from making their cursors
/// interchangeable. Neither component is a credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSearchCursorNamespace {
    store_id: String,
    policy_id: String,
}

impl ProtocolSearchCursorNamespace {
    pub fn new(
        store_id: impl Into<String>,
        policy_id: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let store_id = validate_component(store_id.into(), "protocol_search_cursor_store_id")?;
        let policy_id = validate_component(
            policy_id.into(),
            "protocol_search_cursor_authorization_policy_id",
        )?;
        Ok(Self {
            store_id,
            policy_id,
        })
    }

    pub(crate) fn store_id(&self) -> &str {
        &self.store_id
    }

    pub(crate) fn policy_id(&self) -> &str {
        &self.policy_id
    }
}

fn validate_component(value: String, field: &'static str) -> Result<String, DomainError> {
    if value.is_empty()
        || value.len() > MAX_COMPONENT_BYTES
        || value.chars().any(char::is_control)
        || value.trim() != value
    {
        return Err(DomainError::InvalidCharacters { field });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_control_padded_and_oversized_components() {
        assert!(ProtocolSearchCursorNamespace::new("", "policy").is_err());
        assert!(ProtocolSearchCursorNamespace::new("store", " policy").is_err());
        assert!(ProtocolSearchCursorNamespace::new("store\n", "policy").is_err());
        assert!(ProtocolSearchCursorNamespace::new("x".repeat(257), "policy").is_err());
    }
}
