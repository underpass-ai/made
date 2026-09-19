use made_core::DomainError;

pub(super) fn optional(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

pub(super) fn required(name: &'static str) -> Result<String, DomainError> {
    optional(name).ok_or_else(|| invalid(format!("{name} is required when workers are enabled")))
}

pub(super) fn env_bool(name: &'static str, default: bool) -> Result<bool, DomainError> {
    match optional(name).as_deref() {
        None => Ok(default),
        Some("true" | "1") => Ok(true),
        Some("false" | "0") => Ok(false),
        Some(_) => Err(invalid(format!("{name} must be true or false"))),
    }
}

pub(super) fn env_parse<T>(name: &'static str, default: T) -> Result<T, DomainError>
where
    T: std::str::FromStr,
{
    optional(name).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| invalid(format!("{name} has an invalid value")))
    })
}

pub(super) fn invalid(reason: impl Into<String>) -> DomainError {
    DomainError::InvalidDocument {
        reason: reason.into(),
    }
}
