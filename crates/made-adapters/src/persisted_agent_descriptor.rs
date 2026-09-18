use made_core::error::DomainError;
use made_core::ports::AgentDescriptor;

/// Durable provider configuration contains only supported construction options.
/// Credentials belong to the factory's host configuration, including URL auth.
pub(crate) fn validate(descriptor: &AgentDescriptor) -> Result<(), DomainError> {
    for (key, value) in descriptor.attributes.as_map() {
        let valid = match key.as_str() {
            "provider.model" => value
                .as_str()
                .is_some_and(|model| !model.trim().is_empty() && model.len() <= 4096),
            "provider.max_tokens" | "provider.timeout_secs" => value
                .as_u64()
                .is_some_and(|n| n > 0 && u32::try_from(n).is_ok()),
            "provider.endpoint" => value.as_str().is_some_and(|endpoint| {
                url::Url::parse(endpoint).is_ok_and(|url| {
                    matches!(url.scheme(), "http" | "https")
                        && url.host_str().is_some()
                        && url.username().is_empty()
                        && url.password().is_none()
                        && url.query().is_none()
                        && url.fragment().is_none()
                })
            }),
            _ => false,
        };
        if !valid {
            return Err(DomainError::InvariantViolated {
                reason: "durable agent descriptors accept only non-secret provider model, endpoint, max_tokens and timeout_secs; configure credentials on the host",
            });
        }
    }
    Ok(())
}
