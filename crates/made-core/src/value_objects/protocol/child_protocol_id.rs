use sha2::{Digest, Sha256};

use crate::error::DomainError;

use super::{ChildGroupId, ChildPosition, ProtocolId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildProtocolId(ProtocolId);

impl ChildProtocolId {
    pub fn derive(group_id: &ChildGroupId, position: ChildPosition) -> Result<Self, DomainError> {
        let mut digest = Sha256::new();
        digest.update(b"made.child-protocol.v1\0");
        for part in [group_id.as_str().to_owned(), position.get().to_string()] {
            digest.update((part.len() as u64).to_be_bytes());
            digest.update(part.as_bytes());
        }
        ProtocolId::new(format!("child-{:x}", digest.finalize())).map(Self)
    }
    #[must_use]
    pub fn as_protocol_id(&self) -> &ProtocolId {
        &self.0
    }
    #[must_use]
    pub fn into_protocol_id(self) -> ProtocolId {
        self.0
    }
}
