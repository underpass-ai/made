use std::fmt;

/// The vocabulary every tool failure is reported in.
///
/// Four of the five words are `made-api`'s `ApiError` (ADR-004), and
/// they are there for the same reason: each names a different remedy.
/// Waiting is the remedy for [`Self::Unavailable`], asking for
/// something else is the remedy for [`Self::NotFound`], reading the
/// session again and calling once more is the remedy for
/// [`Self::Conflict`], and [`Self::Refused`] means the engine looked at
/// the call and said no — repeating it unchanged earns the same answer.
///
/// [`Self::InvalidRequest`] is the fifth because an MCP tool is called
/// with arguments a published schema describes, and "these arguments do
/// not fit the schema" is not the engine refusing the work: the caller
/// can fix it without knowing anything about the session. The same word
/// on both backends, because a client that has to know which engine
/// answered in order to read the failure is the divergence this closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolErrorCode {
    Unavailable,
    NotFound,
    /// Somebody else wrote to what this call was writing to.
    ///
    /// `DomainError::Conflict` used to arrive here as
    /// [`Self::Refused`] — through `Status::aborted` on one arm and
    /// directly on the other — which told every client that a lost race
    /// was not worth repeating, against what `domain_error_to_status`
    /// and `AppendOutcome` both say about it. A race is the one failure
    /// that repeating unchanged can win.
    Conflict,
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
            Self::Conflict => "conflict",
            Self::Refused => "refused",
            Self::InvalidRequest => "invalid_request",
        }
    }

    /// Whether trying again, unchanged, could plausibly succeed.
    ///
    /// Published on the error rather than left to the caller, exactly as
    /// `ApiError::is_transient` is, because a caller keeping its own
    /// table of which failures are worth retrying goes stale the first
    /// time this vocabulary grows — which is precisely what happened
    /// when `conflict` joined it.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::Unavailable | Self::Conflict)
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

    /// What can change without the caller changing anything is worth
    /// asking again; what cannot, is not.
    #[test]
    fn an_engine_out_of_reach_and_a_lost_race_invite_a_retry() {
        assert!(ToolErrorCode::Unavailable.is_retryable());
        assert!(
            ToolErrorCode::Conflict.is_retryable(),
            "a race is the one failure repeating unchanged can win"
        );
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
        assert_eq!(ToolErrorCode::Conflict.as_str(), "conflict");
        assert_eq!(ToolErrorCode::Refused.as_str(), "refused");
        assert_eq!(ToolErrorCode::InvalidRequest.as_str(), "invalid_request");
    }
}
