//! The definition behind a protocol, for readings that need one.

use async_trait::async_trait;
use made_core::entities::ProtocolDefinition;
use made_core::error::DomainError;
use made_core::value_objects::ProtocolId;

/// Resolves the definition one running protocol is bound to.
///
/// A port of the attention module's own rather than one of the
/// engine's, and asked one protocol at a time. Two readings need a
/// definition — whether a transition entered a state a person has to
/// answer for, and nothing else yet — and both are about the record in
/// front of them. Resolving lazily, per record, is what keeps the
/// projection composable: the projection is built before the stream
/// that resolves definitions exists, and a lookup that had to be ready
/// at construction time would have forced the two into one order.
///
/// `None` is an answer, not a failure: a protocol this build cannot
/// resolve produces no reading, and the round walks on.
#[async_trait]
pub trait ProtocolDefinitionLookup: Send + Sync + std::fmt::Debug {
    async fn definition_of(
        &self,
        protocol_id: &ProtocolId,
    ) -> Result<Option<ProtocolDefinition>, DomainError>;
}
