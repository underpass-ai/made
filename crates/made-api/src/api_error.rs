use serde::{Deserialize, Serialize};

/// How this contract fails.
///
/// Four shapes, because a consumer acts differently on each: waiting is a
/// remedy for `Unavailable`, asking for something else is the remedy for
/// `CeremonyNotFound`, reading the session again and trying once more is the
/// remedy for `Conflict`, and `Refused` means the engine looked at the request
/// and said no — retrying it unchanged will not change the answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum ApiError {
    #[error("the ceremony engine is unavailable: {reason}")]
    Unavailable { reason: String },

    #[error("no ceremony named `{ceremony_id}`")]
    CeremonyNotFound { ceremony_id: String },

    /// Somebody else wrote to what this call was writing to.
    ///
    /// Distinct from `Refused` because the remedies are opposites: a
    /// refusal will answer the same way however often it is asked, while a
    /// lost race is worth reading again and repeating. Folding the two
    /// together makes a consumer either give up on races it would have won
    /// or hammer a call that will never succeed.
    #[error("the ceremony engine lost a race on {what}; read it again and retry")]
    Conflict { what: String },

    #[error("the ceremony engine refused: {reason}")]
    Refused { reason: String },
}

impl ApiError {
    /// Whether trying again, unchanged, could plausibly succeed.
    ///
    /// Published on the error rather than left to the consumer, because a
    /// consumer keeping its own table of which errors are worth retrying goes
    /// stale the first time this enum grows.
    #[must_use]
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::Unavailable { .. } | Self::Conflict { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_can_change_by_itself_invites_a_retry() {
        assert!(ApiError::Unavailable {
            reason: "starting".to_owned()
        }
        .is_transient());
        assert!(
            ApiError::Conflict {
                what: "ceremony_instance".to_owned()
            }
            .is_transient(),
            "a lost race is the one failure repeating unchanged can win"
        );
        assert!(!ApiError::CeremonyNotFound {
            ceremony_id: "c-1".to_owned()
        }
        .is_transient());
        assert!(
            !ApiError::Refused {
                reason: "unbound definition".to_owned()
            }
            .is_transient(),
            "retrying a refusal unchanged asks the same question and earns the \
             same answer"
        );
    }
}
