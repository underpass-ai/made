use super::define_label;
/// A non-secret model label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderModel(pub(super) String);
define_label!(ProviderModel, "model");
