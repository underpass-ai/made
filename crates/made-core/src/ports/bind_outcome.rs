use crate::value_objects::IntegratorBinding;

/// What the store did with an offered integrator binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindOutcome {
    /// The scope had nobody; this binding holds it now.
    Bound(IntegratorBinding),
    /// The same binding was already there; nothing changed.
    AlreadyBound(IntegratorBinding),
    /// A different binding was displaced, and the fence went up.
    Replaced {
        previous: Box<IntegratorBinding>,
        current: Box<IntegratorBinding>,
    },
    /// A different binding is live and the caller did not ask to replace it.
    AlreadyExists { existing: Box<IntegratorBinding> },
}

impl BindOutcome {
    /// The binding now in force, when the call produced one.
    #[must_use]
    pub const fn current(&self) -> Option<&IntegratorBinding> {
        match self {
            Self::Bound(binding) | Self::AlreadyBound(binding) => Some(binding),
            Self::Replaced { current, .. } => Some(current),
            Self::AlreadyExists { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_bound(&self) -> bool {
        !matches!(self, Self::AlreadyExists { .. })
    }
}
