use made_core::value_objects::{
    AuthorizationTargetDigest, CeremonyIdPrefix, CeremonyInstancePageLimit, CeremonyLifecyclePhase,
};

use crate::usecases::CeremonySearchCursor;

/// Validated application query behind the public paginated listing.
#[derive(Debug, Clone)]
pub struct SearchCeremonyInstancesInput {
    cursor: Option<CeremonySearchCursor>,
    limit: CeremonyInstancePageLimit,
    id_prefix: Option<CeremonyIdPrefix>,
    lifecycle: Option<CeremonyLifecyclePhase>,
}

impl SearchCeremonyInstancesInput {
    #[must_use]
    pub fn new(
        cursor: Option<CeremonySearchCursor>,
        limit: CeremonyInstancePageLimit,
        id_prefix: Option<CeremonyIdPrefix>,
        lifecycle: Option<CeremonyLifecyclePhase>,
    ) -> Self {
        Self {
            cursor,
            limit,
            id_prefix,
            lifecycle,
        }
    }

    #[must_use]
    pub fn cursor(&self) -> Option<&CeremonySearchCursor> {
        self.cursor.as_ref()
    }

    #[must_use]
    pub const fn limit(&self) -> CeremonyInstancePageLimit {
        self.limit
    }

    #[must_use]
    pub fn id_prefix(&self) -> Option<&CeremonyIdPrefix> {
        self.id_prefix.as_ref()
    }

    #[must_use]
    pub const fn lifecycle(&self) -> Option<CeremonyLifecyclePhase> {
        self.lifecycle
    }

    /// Canonical semantic request digest used by transport-neutral authorization.
    #[must_use]
    pub fn authorization_target_digest(&self) -> AuthorizationTargetDigest {
        let mut bytes = b"underpass.made.search-ceremony-instances.v1\0".to_vec();
        push_optional(
            &mut bytes,
            self.cursor.as_ref().map(CeremonySearchCursor::as_str),
        );
        bytes.extend_from_slice(&(self.limit.value() as u64).to_be_bytes());
        push_optional(
            &mut bytes,
            self.id_prefix.as_ref().map(CeremonyIdPrefix::as_str),
        );
        bytes.push(match self.lifecycle {
            None => 0,
            Some(CeremonyLifecyclePhase::Running) => 1,
            Some(CeremonyLifecyclePhase::Paused) => 2,
            Some(CeremonyLifecyclePhase::Ended) => 3,
        });
        AuthorizationTargetDigest::for_bytes(&bytes)
    }
}

fn push_optional(bytes: &mut Vec<u8>, value: Option<&str>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
            bytes.extend_from_slice(value.as_bytes());
        }
    }
}
