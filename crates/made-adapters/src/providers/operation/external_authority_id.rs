use super::define_label;
/// Identifier of a system that is authoritative for an observation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalAuthorityId(pub(super) String);
define_label!(ExternalAuthorityId, "external_authority");
