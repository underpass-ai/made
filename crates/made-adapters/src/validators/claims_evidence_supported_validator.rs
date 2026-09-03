use std::sync::Arc;

use made_core::ports::EvidenceSupportJudgePort;

pub struct ClaimsEvidenceSupportedValidator {
    pub(super) judge: Option<Arc<dyn EvidenceSupportJudgePort>>,
}
