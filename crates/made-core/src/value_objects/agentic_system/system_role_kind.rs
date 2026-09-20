use std::fmt;

use serde::{Deserialize, Serialize};

/// What kind of part a business role plays in the system.
///
/// The kinds are about authority and not about job titles: who drives
/// the work, who does it, who reads it critically, who signs it off,
/// and who only watches. Validation leans on them — an integrator must
/// be an `Integrator`, and an independence rule is about a `Reviewer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemRoleKind {
    Integrator,
    Contributor,
    Reviewer,
    Approver,
    Observer,
}

impl SystemRoleKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Integrator => "integrator",
            Self::Contributor => "contributor",
            Self::Reviewer => "reviewer",
            Self::Approver => "approver",
            Self::Observer => "observer",
        }
    }

    /// Whether this kind may be named as the system's integrator.
    #[must_use]
    pub const fn drives_the_system(self) -> bool {
        matches!(self, Self::Integrator)
    }
}

impl fmt::Display for SystemRoleKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_integrator_kind_drives_the_system() {
        assert!(SystemRoleKind::Integrator.drives_the_system());
        assert!(!SystemRoleKind::Approver.drives_the_system());
        assert_eq!(SystemRoleKind::Observer.to_string(), "observer");
    }
}
