use std::fmt;

/// The vocabulary every tool failure is reported in.
///
/// Three of the four words are `made-api`'s `ApiError` (ADR-004), and
/// they are there for the same reason: each names a different remedy.
/// Waiting is the remedy for [`Self::Unavailable`], asking for
/// something else is the remedy for [`Self::NotFound`], and
/// [`Self::Refused`] means the engine looked at the call and said no —
/// repeating it unchanged earns the same answer.
///
/// [`Self::InvalidRequest`] is the fourth because an MCP tool is called
/// with arguments a published schema describes, and "these arguments do
/// not fit the schema" is not the engine refusing the work: the caller
/// can fix it without knowing anything about the session. The same word
/// on both backends, because a client that has to know which engine
/// answered in order to read the failure is the divergence this closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolErrorCode {
    Unavailable,
    NotFound,
    Refused,
    InvalidRequest,
}

impl ToolErrorCode {
    /// The wire spelling. Lower snake case, as the codes are read by
    /// machines first.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::NotFound => "not_found",
            Self::Refused => "refused",
            Self::InvalidRequest => "invalid_request",
        }
    }

    /// Whether trying again, unchanged, could plausibly succeed.
    ///
    /// Published on the error rather than left to the caller, exactly as
    /// `ApiError::is_transient` is, because a caller keeping its own
    /// table of which failures are worth retrying goes stale the first
    /// time this vocabulary grows.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Unavailable)
    }
}

impl fmt::Display for ToolErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unavailability_invites_a_retry() {
        assert!(ToolErrorCode::Unavailable.is_retryable());
        for code in [
            ToolErrorCode::NotFound,
            ToolErrorCode::Refused,
            ToolErrorCode::InvalidRequest,
        ] {
            assert!(
                !code.is_retryable(),
                "{code} does not become true by being asked again"
            );
        }
    }

    #[test]
    fn the_wire_spelling_is_lower_snake_case() {
        assert_eq!(ToolErrorCode::Unavailable.as_str(), "unavailable");
        assert_eq!(ToolErrorCode::NotFound.as_str(), "not_found");
        assert_eq!(ToolErrorCode::Refused.as_str(), "refused");
        assert_eq!(ToolErrorCode::InvalidRequest.as_str(), "invalid_request");
    }
}
