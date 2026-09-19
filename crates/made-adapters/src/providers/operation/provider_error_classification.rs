use made_core::value_objects::LlmErrorKind;

/// Low-cardinality error classification; raw provider bodies are intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderErrorClassification {
    Unauthorized,
    RateLimited,
    BadRequest,
    Upstream,
    MalformedResponse,
    EmptyResponse,
    Timeout,
    Transport,
    Cancelled,
    Unknown,
}

impl From<LlmErrorKind> for ProviderErrorClassification {
    fn from(value: LlmErrorKind) -> Self {
        match value {
            LlmErrorKind::Unauthorized => Self::Unauthorized,
            LlmErrorKind::RateLimited => Self::RateLimited,
            LlmErrorKind::BadRequest => Self::BadRequest,
            LlmErrorKind::UpstreamError => Self::Upstream,
            LlmErrorKind::MalformedBody => Self::MalformedResponse,
            LlmErrorKind::EmptyContent => Self::EmptyResponse,
            LlmErrorKind::Timeout => Self::Timeout,
            LlmErrorKind::Transport => Self::Transport,
        }
    }
}
