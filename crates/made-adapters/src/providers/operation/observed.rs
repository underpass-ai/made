use super::{ExternalAuthorityId, ObservationDeclarer, ObservationOrigin, ProviderContractError};

/// A value paired with the origin that gave it meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed<T> {
    value: T,
    origin: ObservationOrigin,
}

impl<T> Observed<T> {
    #[must_use]
    pub const fn declared_by_provider(value: T) -> Self {
        Self {
            value,
            origin: ObservationOrigin::Declared {
                by: ObservationDeclarer::Provider,
            },
        }
    }
    #[must_use]
    pub const fn declared_by_adapter(value: T) -> Self {
        Self {
            value,
            origin: ObservationOrigin::Declared {
                by: ObservationDeclarer::Adapter,
            },
        }
    }
    #[must_use]
    pub const fn declared_by_fixture(value: T) -> Self {
        Self {
            value,
            origin: ObservationOrigin::Declared {
                by: ObservationDeclarer::Fixture,
            },
        }
    }
    pub fn from_external_authority(
        value: T,
        authority: impl Into<String>,
    ) -> Result<Self, ProviderContractError> {
        Ok(Self {
            value,
            origin: ObservationOrigin::ExternalAuthority {
                authority: ExternalAuthorityId::new(authority)?,
            },
        })
    }
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }
    #[must_use]
    pub const fn origin(&self) -> &ObservationOrigin {
        &self.origin
    }
    #[must_use]
    pub const fn is_externally_authoritative(&self) -> bool {
        matches!(self.origin, ObservationOrigin::ExternalAuthority { .. })
    }
    #[must_use]
    pub fn into_parts(self) -> (T, ObservationOrigin) {
        (self.value, self.origin)
    }
}
