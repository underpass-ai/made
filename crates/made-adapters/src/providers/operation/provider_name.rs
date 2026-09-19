use super::define_label;
/// A non-secret provider label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderName(pub(super) String);
define_label!(ProviderName, "provider");
