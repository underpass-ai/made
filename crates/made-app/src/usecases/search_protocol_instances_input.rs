use made_core::value_objects::{
    AuthorizationTargetDigest, ProtocolIdPrefix, ProtocolInstancePageLimit, ProtocolLifecyclePhase,
};

use crate::usecases::ProtocolSearchCursor;

/// Validated application query behind the public paginated listing.
#[derive(Debug, Clone)]
pub struct SearchProtocolInstancesInput {
    cursor: Option<ProtocolSearchCursor>,
    limit: ProtocolInstancePageLimit,
    id_prefix: Option<ProtocolIdPrefix>,
    lifecycle: Option<ProtocolLifecyclePhase>,
}

impl SearchProtocolInstancesInput {
    #[must_use]
    pub fn new(
        cursor: Option<ProtocolSearchCursor>,
        limit: ProtocolInstancePageLimit,
        id_prefix: Option<ProtocolIdPrefix>,
        lifecycle: Option<ProtocolLifecyclePhase>,
    ) -> Self {
        Self {
            cursor,
            limit,
            id_prefix,
            lifecycle,
        }
    }

    #[must_use]
    pub fn cursor(&self) -> Option<&ProtocolSearchCursor> {
        self.cursor.as_ref()
    }

    #[must_use]
    pub const fn limit(&self) -> ProtocolInstancePageLimit {
        self.limit
    }

    #[must_use]
    pub fn id_prefix(&self) -> Option<&ProtocolIdPrefix> {
        self.id_prefix.as_ref()
    }

    #[must_use]
    pub const fn lifecycle(&self) -> Option<ProtocolLifecyclePhase> {
        self.lifecycle
    }

    /// Canonical semantic request digest used by transport-neutral authorization.
    #[must_use]
    pub fn authorization_target_digest(&self) -> AuthorizationTargetDigest {
        let mut bytes = b"underpass.made.search-protocol-instances.v1\0".to_vec();
        push_optional(
            &mut bytes,
            self.cursor.as_ref().map(ProtocolSearchCursor::as_str),
        );
        bytes.extend_from_slice(&(self.limit.value() as u64).to_be_bytes());
        push_optional(
            &mut bytes,
            self.id_prefix.as_ref().map(ProtocolIdPrefix::as_str),
        );
        bytes.push(match self.lifecycle {
            None => 0,
            Some(ProtocolLifecyclePhase::Running) => 1,
            Some(ProtocolLifecyclePhase::Paused) => 2,
            Some(ProtocolLifecyclePhase::Ended) => 3,
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
