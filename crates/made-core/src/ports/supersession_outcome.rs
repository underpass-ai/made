use crate::value_objects::{HostDeliveryId, HostDeliveryRecord};

/// What replacing a destination did to the work addressed to it.
///
/// Both halves matter to a caller. The superseded identifiers are what
/// an operator sees when asking why a question was never answered; the
/// replacements are the deliveries a caller still has to push, and a
/// call that returned only a count could not do either.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupersessionOutcome {
    superseded: Vec<HostDeliveryId>,
    replacements: Vec<HostDeliveryRecord>,
}

impl SupersessionOutcome {
    #[must_use]
    pub const fn new(
        superseded: Vec<HostDeliveryId>,
        replacements: Vec<HostDeliveryRecord>,
    ) -> Self {
        Self {
            superseded,
            replacements,
        }
    }

    /// The deliveries that were closed because their destination went.
    #[must_use]
    pub fn superseded(&self) -> &[HostDeliveryId] {
        &self.superseded
    }

    /// The deliveries opened in their place, when the work follows.
    #[must_use]
    pub fn replacements(&self) -> &[HostDeliveryRecord] {
        &self.replacements
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.superseded.is_empty() && self.replacements.is_empty()
    }
}
