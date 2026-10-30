use serde::{Deserialize, Serialize};

use crate::value_objects::ProtocolGuardDeferral;

/// A human guard was left undecided, on purpose, with the statement,
/// reason and reconsideration conditions the deferral carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HumanDeferralRecorded {
    pub deferral: ProtocolGuardDeferral,
}
