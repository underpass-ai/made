use made_core::value_objects::AuthorizationDecisionId;
use serde::{Deserialize, Serialize};

use super::maintenance_command::MaintenanceCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRequest {
    pub command: MaintenanceCommand,
    pub approval_decision_id: Option<AuthorizationDecisionId>,
}
