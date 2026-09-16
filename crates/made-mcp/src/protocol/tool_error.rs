use std::fmt;

use serde_json::{json, Value};

use super::ToolErrorCode;

/// One failed tool call, in the one envelope both backends speak.
///
/// Every tool error crosses the wire as `{code, message, retryable}` in
/// the result's structured content, whichever backend produced it. Before
/// this, the gRPC arm answered `"gRPC {code}: {message}"` — the transport
/// leaking into the contract — and the in-process arm answered whatever
/// `DomainError`'s `Display` said, so a client could not classify either
/// without parsing prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    code: ToolErrorCode,
    message: String,
}

impl ToolError {
    /// The engine could not be reached. Waiting is the remedy.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ToolErrorCode::Unavailable, message)
    }

    /// What the call named is not there. Asking for something else is
    /// the remedy.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ToolErrorCode::NotFound, message)
    }

    /// The engine looked at the call and said no.
    pub fn refused(message: impl Into<String>) -> Self {
        Self::new(ToolErrorCode::Refused, message)
    }

    /// The arguments do not fit the tool's schema.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(ToolErrorCode::InvalidRequest, message)
    }

    fn new(code: ToolErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> ToolErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether trying again, unchanged, could plausibly succeed.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.code.is_retryable()
    }

    /// The envelope itself, as machine-readable structured content.
    #[must_use]
    pub fn to_structured(&self) -> Value {
        json!({
            "code": self.code.as_str(),
            "message": self.message,
            "retryable": self.is_retryable(),
        })
    }
}

/// The code before the message, so a host that reads only the text
/// content still sees which of the four this was. The transport never
/// appears: `unavailable` says the same thing whether the engine was
/// across a network or failed to open in this process.
impl fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ToolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_envelope_carries_code_message_and_retryability() {
        let error = ToolError::unavailable("the endpoint refused the connection");
        assert_eq!(
            error.to_structured(),
            json!({
                "code": "unavailable",
                "message": "the endpoint refused the connection",
                "retryable": true,
            })
        );
    }

    #[test]
    fn a_refusal_is_not_worth_repeating() {
        let error = ToolError::refused("no satisfied ceremony transition is available");
        assert_eq!(error.to_structured()["retryable"], json!(false));
        assert_eq!(error.code(), ToolErrorCode::Refused);
    }

    #[test]
    fn the_text_rendering_names_the_code_and_never_the_transport() {
        let error = ToolError::not_found("no ceremony named `c-1`");
        assert_eq!(error.to_string(), "not_found: no ceremony named `c-1`");
        assert!(!error.to_string().contains("gRPC"));
    }
}
