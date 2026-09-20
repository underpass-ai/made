use serde::{Deserialize, Serialize};

use super::{LoopRounds, SystemCeremonyId};

/// What makes one composed ceremony start.
///
/// `Loop` is the only way a design may contain a cycle: it names the
/// composition the cycle runs back to and how many times, so a system
/// that sends work round for revision is expressible and a system that
/// could never finish is not.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CeremonyActivation {
    /// Somebody starts it.
    Manual,
    /// It starts once everything it depends on has completed.
    AfterDependencies,
    /// It runs again after `after`, up to `max_rounds` times.
    Loop {
        after: SystemCeremonyId,
        max_rounds: LoopRounds,
    },
}

impl CeremonyActivation {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::AfterDependencies => "after_dependencies",
            Self::Loop { .. } => "loop",
        }
    }

    /// The composition this one loops back to, if it loops.
    #[must_use]
    pub const fn loops_after(&self) -> Option<&SystemCeremonyId> {
        match self {
            Self::Manual | Self::AfterDependencies => None,
            Self::Loop { after, .. } => Some(after),
        }
    }

    #[must_use]
    pub const fn max_rounds(&self) -> Option<LoopRounds> {
        match self {
            Self::Manual | Self::AfterDependencies => None,
            Self::Loop { max_rounds, .. } => Some(*max_rounds),
        }
    }

    /// Whether a run starts this composition without being asked.
    #[must_use]
    pub const fn is_automatic(&self) -> bool {
        !matches!(self, Self::Manual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_loop_names_where_it_goes_back_to() {
        let looping = CeremonyActivation::Loop {
            after: SystemCeremonyId::new("drafting").unwrap(),
            max_rounds: LoopRounds::new(2).unwrap(),
        };

        assert_eq!(looping.loops_after().unwrap().as_str(), "drafting");
        assert_eq!(looping.max_rounds().unwrap().get(), 2);
        assert!(CeremonyActivation::Manual.loops_after().is_none());
        assert!(!CeremonyActivation::Manual.is_automatic());
        assert!(CeremonyActivation::AfterDependencies.is_automatic());
    }
}
