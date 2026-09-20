/// Whether a listing wants everything, or only what is still owed.
///
/// Named rather than a bare flag because the two answers are different
/// questions: one asks what a ceremony has been asked, and the other
/// asks what nobody has dealt with. A reader of a call site should not
/// have to remember which way round `true` meant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterventionResolutionFilter {
    /// Every item, resolved or not.
    #[default]
    Any,
    /// Only items somebody still owes something.
    UnresolvedOnly,
}

impl InterventionResolutionFilter {
    /// The filter a boolean flag on a wire means.
    #[must_use]
    pub const fn from_unresolved_only(unresolved_only: bool) -> Self {
        if unresolved_only {
            Self::UnresolvedOnly
        } else {
            Self::Any
        }
    }

    #[must_use]
    pub const fn admits_resolved(self) -> bool {
        matches!(self, Self::Any)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_narrow_filter_hides_what_is_settled() {
        assert!(InterventionResolutionFilter::default().admits_resolved());
        assert!(!InterventionResolutionFilter::from_unresolved_only(true).admits_resolved());
        assert!(InterventionResolutionFilter::from_unresolved_only(false).admits_resolved());
    }
}
