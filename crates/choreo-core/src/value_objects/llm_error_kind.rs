//! [`LlmErrorKind`] — a low-cardinality classification of an LLM call
//! failure.
//!
//! A single dimension for error metrics across every LLM-backed adapter
//! (the judge today; provider agents in later slices). The variants name
//! *what went wrong* at a granularity that demands different operator
//! responses — a `RateLimited` is backpressure, an `Unauthorized` is a
//! credential rotation, a `Timeout` is saturation — so the same eight
//! buckets serve both alerting and triage.

/// How an LLM call failed, as a stable metric label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LlmErrorKind {
    /// 401/403 — credentials missing, wrong, or revoked.
    Unauthorized,
    /// 429 — provider-side rate limiting / backpressure.
    RateLimited,
    /// Other 4xx — a malformed or rejected request.
    BadRequest,
    /// 5xx or an otherwise unclassified upstream failure.
    UpstreamError,
    /// The response body could not be parsed into the expected shape.
    MalformedBody,
    /// The response parsed but carried no usable content.
    EmptyContent,
    /// The call exceeded its deadline.
    Timeout,
    /// A connection-level failure before any HTTP status was seen.
    Transport,
}

impl LlmErrorKind {
    /// Classify an HTTP status code, mirroring the adapters' error
    /// mapping. Takes a raw `u16` so the core stays free of any HTTP
    /// client dependency.
    #[must_use]
    pub const fn from_status(status: u16) -> Self {
        match status {
            401 | 403 => Self::Unauthorized,
            429 => Self::RateLimited,
            400..=499 => Self::BadRequest,
            _ => Self::UpstreamError,
        }
    }

    /// Stable, low-cardinality label value for metrics exposition. These
    /// strings are part of the metric contract — dashboards and alerts
    /// match on them, so they must not change.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::RateLimited => "rate_limited",
            Self::BadRequest => "bad_request",
            Self::UpstreamError => "upstream_error",
            Self::MalformedBody => "malformed_body",
            Self::EmptyContent => "empty_content",
            Self::Timeout => "timeout",
            Self::Transport => "transport",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_classification_matches_the_adapter_mapping() {
        assert_eq!(LlmErrorKind::from_status(401), LlmErrorKind::Unauthorized);
        assert_eq!(LlmErrorKind::from_status(403), LlmErrorKind::Unauthorized);
        assert_eq!(LlmErrorKind::from_status(429), LlmErrorKind::RateLimited);
        assert_eq!(LlmErrorKind::from_status(404), LlmErrorKind::BadRequest);
        assert_eq!(LlmErrorKind::from_status(400), LlmErrorKind::BadRequest);
        assert_eq!(LlmErrorKind::from_status(500), LlmErrorKind::UpstreamError);
        assert_eq!(LlmErrorKind::from_status(503), LlmErrorKind::UpstreamError);
    }

    #[test]
    fn labels_are_distinct_and_stable() {
        let all = [
            LlmErrorKind::Unauthorized,
            LlmErrorKind::RateLimited,
            LlmErrorKind::BadRequest,
            LlmErrorKind::UpstreamError,
            LlmErrorKind::MalformedBody,
            LlmErrorKind::EmptyContent,
            LlmErrorKind::Timeout,
            LlmErrorKind::Transport,
        ];
        let labels: std::collections::BTreeSet<&str> =
            all.iter().map(|kind| kind.as_label()).collect();
        assert_eq!(labels.len(), all.len(), "labels must be unique");
    }
}
