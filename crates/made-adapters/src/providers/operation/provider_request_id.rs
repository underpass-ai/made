use super::define_label;
/// Caller-assigned request/correlation id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderRequestId(pub(super) String);
define_label!(ProviderRequestId, "request_id");
