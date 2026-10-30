use serde::{Deserialize, Serialize};

/// Whether a validation finding blocks a protocol definition or only
/// warns about it.
///
/// A definition with any [`ProtocolValidationSeverity::Error`] finding
/// cannot be constructed and must not be published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolValidationSeverity {
    Error,
    Warning,
}

impl ProtocolValidationSeverity {
    #[must_use]
    pub fn is_error(self) -> bool {
        self == Self::Error
    }

    #[must_use]
    pub fn is_warning(self) -> bool {
        self == Self::Warning
    }
}
