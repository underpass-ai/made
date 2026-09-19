use super::define_label;
/// Stable semantic provider operation id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderOperationId(pub(super) String);
define_label!(ProviderOperationId, "operation_id");
