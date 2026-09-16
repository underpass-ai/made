/// How an engine describes its own condition.
///
/// The three words `GetStatusResponse.health` has always named, as a
/// type instead of a comment above a string. Both compositions answer
/// through it, so neither can invent a fourth word and neither can
/// spell one of these three differently from the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceHealth {
    /// Every dependency the engine needs is answering.
    #[default]
    Healthy,
    /// The engine is answering, but something it depends on is not.
    Degraded,
    /// The engine cannot do its work.
    Unhealthy,
}

impl ServiceHealth {
    /// The wire spelling, lower-case, as the contract declares it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_condition_has_the_spelling_the_contract_declares() {
        assert_eq!(ServiceHealth::Healthy.as_str(), "healthy");
        assert_eq!(ServiceHealth::Degraded.as_str(), "degraded");
        assert_eq!(ServiceHealth::Unhealthy.as_str(), "unhealthy");
    }

    #[test]
    fn an_engine_that_says_nothing_else_is_healthy() {
        assert_eq!(ServiceHealth::default(), ServiceHealth::Healthy);
    }
}
