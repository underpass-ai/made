use crate::error::DomainError;
use crate::value_objects::{HostDeliveryTarget, HostDeliveryTargetKey};

/// Which destinations a caller is asking about.
///
/// A list rather than one destination, because the common question is
/// not one: a host asks for what is addressed to its own execution
/// *and* for what is waiting on the role it is filling, and asking
/// twice would hand out two leases where the caller wanted one batch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum HostDeliveryTargetFilter {
    /// Every destination.
    #[default]
    Any,
    /// Only these, in the order the caller listed them.
    AnyOf(Vec<HostDeliveryTarget>),
}

impl HostDeliveryTargetFilter {
    /// Restrict to one or more destinations.
    pub fn any_of(
        targets: impl IntoIterator<Item = HostDeliveryTarget>,
    ) -> Result<Self, DomainError> {
        let targets: Vec<HostDeliveryTarget> = targets.into_iter().collect();
        if targets.is_empty() {
            return Err(DomainError::EmptyCollection {
                field: "host_delivery_target_filter",
            });
        }
        Ok(Self::AnyOf(targets))
    }

    /// Whether a destination is one of the ones asked about.
    #[must_use]
    pub fn admits(&self, target: &HostDeliveryTarget) -> bool {
        match self {
            Self::Any => true,
            Self::AnyOf(targets) => targets.contains(target),
        }
    }

    /// The keys this filter selects, for a store that scans by key.
    #[must_use]
    pub fn keys(&self) -> Option<Vec<HostDeliveryTargetKey>> {
        match self {
            Self::Any => None,
            Self::AnyOf(targets) => {
                Some(targets.iter().map(HostDeliveryTarget::target_key).collect())
            }
        }
    }
}
