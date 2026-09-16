//! One step handler, both engines.
//!
//! The parity test used to compare shapes and skip `output`, `details`,
//! `context` and `evidence_pack`, because the server deliberated
//! through its executor while the in-process edition ran a handler that
//! does nothing: comparing those fields would have compared the two
//! handlers rather than the contract. Running this one on both arms is
//! what turns them back into fields the test can compare.
//!
//! What it answers is derived from the step alone — its id and the
//! state it belongs to — so two engines executing the same session
//! produce the same output without sharing anything but this type.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use made_core::error::DomainError;
use made_core::ports::{CeremonyStepHandlerPort, CeremonyStepHandlerRequest};
use made_core::value_objects::{Attributes, StepOutput, StepResult};
use serde_json::json;

/// A [`CeremonyStepHandlerPort`] whose answer is a function of the step.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParityStepHandler;

impl ParityStepHandler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// The handler, ready to hand to a builder or a fixture.
    #[must_use]
    pub fn shared() -> Arc<dyn CeremonyStepHandlerPort> {
        Arc::new(Self::new())
    }

    /// What this handler answers for one step. Non-empty on purpose:
    /// an empty output would let the two arms agree about `output` by
    /// having nothing in it to disagree about.
    fn output_for(step_id: &str, state_id: &str) -> Result<StepOutput, DomainError> {
        let attributes = Attributes::new(BTreeMap::from([
            ("handler".to_owned(), json!("parity")),
            ("step".to_owned(), json!(step_id)),
            ("state".to_owned(), json!(state_id)),
            (
                "findings".to_owned(),
                json!([{ "about": step_id, "verdict": "done" }]),
            ),
            (
                "summary".to_owned(),
                json!(format!("`{step_id}` ran in `{state_id}`.")),
            ),
        ]))?;
        Ok(StepOutput::new(attributes))
    }
}

#[async_trait]
impl CeremonyStepHandlerPort for ParityStepHandler {
    async fn execute(
        &self,
        request: CeremonyStepHandlerRequest,
    ) -> Result<StepResult, DomainError> {
        StepResult::completed(Self::output_for(
            request.step_id().as_str(),
            request.current_state().as_str(),
        )?)
    }
}
