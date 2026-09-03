use serde::{Deserialize, Serialize};

/// Wire- and storage-stable structured output format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutputFormat {
    #[default]
    JsonObject,
}

impl OutputFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JsonObject => "json_object",
        }
    }
}
