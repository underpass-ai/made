use serde::{Deserialize, Serialize};

/// How a terminal receipt became bound to the claim that completed a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionReceiptLinkKind {
    Direct,
    Adopted,
}
