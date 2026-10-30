use serde::{Deserialize, Serialize};

/// An external host declaration, never an engine inference of process liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostWorkState {
    HandoffRequested,
    HandoffAcknowledged,
    Quiesced,
    Lost,
}
