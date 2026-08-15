//! [`CeremonyStepContribution`] — one agent-produced intervention in a
//! ceremony's running transcript.
//!
//! A contribution records that a given role, executing a given step,
//! produced a [`StepOutput`]. It is the unit a context store accumulates
//! so that later steps can deliberate with the earlier interventions in
//! view — turning a sequence of isolated steps into one conversation.

use serde::{Deserialize, Serialize};

use super::{RoleId, StepId, StepOutput};

/// One intervention produced during a ceremony step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CeremonyStepContribution {
    step_id: StepId,
    role_id: RoleId,
    output: StepOutput,
}

impl CeremonyStepContribution {
    /// Record the `output` produced by `role_id` while executing `step_id`.
    #[must_use]
    pub fn new(step_id: StepId, role_id: RoleId, output: StepOutput) -> Self {
        Self {
            step_id,
            role_id,
            output,
        }
    }

    /// The step that produced this contribution.
    #[must_use]
    pub fn step_id(&self) -> &StepId {
        &self.step_id
    }

    /// The role that produced this contribution.
    #[must_use]
    pub fn role_id(&self) -> &RoleId {
        &self.role_id
    }

    /// The output the step produced.
    #[must_use]
    pub fn output(&self) -> &StepOutput {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;
    use crate::value_objects::Attributes;

    fn contribution() -> CeremonyStepContribution {
        let output = StepOutput::new(
            Attributes::new(BTreeMap::from([(
                "winner_content".to_owned(),
                json!("We should ship the smaller scope first."),
            )]))
            .unwrap(),
        );
        CeremonyStepContribution::new(
            StepId::new("open_room").unwrap(),
            RoleId::new("FACILITATOR").unwrap(),
            output,
        )
    }

    #[test]
    fn exposes_its_components() {
        let contribution = contribution();

        assert_eq!(contribution.step_id().as_str(), "open_room");
        assert_eq!(contribution.role_id().as_str(), "FACILITATOR");
        assert_eq!(
            contribution
                .output()
                .attributes()
                .get("winner_content")
                .and_then(serde_json::Value::as_str),
            Some("We should ship the smaller scope first.")
        );
    }

    #[test]
    fn serde_roundtrips() {
        let contribution = contribution();
        let json = serde_json::to_string(&contribution).unwrap();
        let restored: CeremonyStepContribution = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, contribution);
    }
}
