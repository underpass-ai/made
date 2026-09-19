use serde::{Deserialize, Serialize};

/// Structured process-group termination evidence.
#[expect(
    clippy::struct_excessive_bools,
    reason = "the receipt records four independent process-termination facts"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalProcessTermination {
    pub requested: bool,
    pub supported: bool,
    pub attempted: bool,
    pub succeeded: bool,
}

impl LocalProcessTermination {
    pub(super) const fn not_requested() -> Self {
        Self {
            requested: false,
            supported: cfg!(unix),
            attempted: false,
            succeeded: false,
        }
    }
}
