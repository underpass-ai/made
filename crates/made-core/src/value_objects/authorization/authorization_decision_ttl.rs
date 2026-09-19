use time::Duration;

use crate::DomainError;

const MAX_SECONDS: u32 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthorizationDecisionTtl(u32);

impl AuthorizationDecisionTtl {
    pub fn from_seconds(seconds: u32) -> Result<Self, DomainError> {
        if seconds == 0 || seconds > MAX_SECONDS {
            return Err(DomainError::OutOfRange {
                field: "authorization_decision_ttl_seconds",
                value: f64::from(seconds),
                min: 1.0,
                max: f64::from(MAX_SECONDS),
            });
        }
        Ok(Self(seconds))
    }

    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::seconds(i64::from(self.0))
    }
}
