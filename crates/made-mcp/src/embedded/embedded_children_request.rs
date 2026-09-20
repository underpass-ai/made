use made_core::value_objects::{ChildGroupId, ChildSpawnCoordinates};
use made_embedded::{EmbeddedCeremonyOperationAuthority, EmbeddedMade};
use serde_json::{json, Value};

use made_app::budgets::BudgetedStepClaimInput;

use super::embedded_budget_fields;
use super::embedded_ceremony_instance_presenter::EmbeddedCeremonyInstancePresenter;
use super::embedded_request_fields::load_instance_definition;
use super::embedded_run_ceremony_step_request::EmbeddedRunCeremonyStepRequest;
use crate::protocol::ToolError;

#[derive(Clone, Debug)]
pub(super) struct EmbeddedPrepareCeremonyChildrenRequest {
    run: EmbeddedRunCeremonyStepRequest,
    reservation: Option<made_core::value_objects::BudgetReservationEstimate>,
}

impl EmbeddedPrepareCeremonyChildrenRequest {
    pub(super) async fn execute(self, made: &EmbeddedMade) -> Result<Value, ToolError> {
        let (definition, instance) = load_instance_definition(made, self.run.ceremony_id()).await?;
        let step_id = self.run.step_id().clone();
        let operations = EmbeddedCeremonyOperationAuthority::for_engine(made);
        let instance = if instance.budget_account_id().is_some() {
            let reservation = self.reservation.ok_or_else(|| {
                ToolError::refused("budgeted child preparation requires a reservation estimate")
            })?;
            // Boxed for the same reason its unbudgeted sibling below
            // is: the composed future is large enough to be worth a
            // heap allocation rather than a frame of this size.
            Box::pin(operations.prepare_budgeted_children(
                BudgetedStepClaimInput::new(self.run.claim_input(&definition)?, reservation),
                step_id.clone(),
                self.run.actor_kind(),
            ))
            .await?
            .instance()
            .clone()
        } else if self.reservation.is_some() {
            return Err(ToolError::invalid_request(
                "budget reservation supplied for an unbudgeted ceremony",
            ));
        } else {
            Box::pin(operations.prepare_unbudgeted_children(self.run.run_input(&definition)?))
                .await?
                .instance()
                .clone()
        };
        let record = instance
            .step_record(&step_id)
            .ok_or_else(|| ToolError::refused("made returned no spawning step record"))?;
        let coordinates = ChildSpawnCoordinates::new(
            step_id,
            instance.current_state_visit(),
            instance.current_state_iteration(),
            record.iteration(),
        );
        let group_id = ChildGroupId::derive(instance.id(), &coordinates);
        let group = instance
            .child_group(&group_id)
            .ok_or_else(|| ToolError::refused("made returned no child spawn group"))?;
        let instance = EmbeddedCeremonyInstancePresenter::present(made, instance.id()).await?;
        Ok(json!({
            "instance": instance,
            "child_group_id": group_id.as_str(),
            "child_ids": group.plan().children().iter().map(|child| child.child_id().as_str()).collect::<Vec<_>>(),
        }))
    }
}

impl TryFrom<&Value> for EmbeddedPrepareCeremonyChildrenRequest {
    type Error = String;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let object = value
            .as_object()
            .ok_or_else(|| "tools/call.arguments must be an object".to_owned())?;
        Ok(Self {
            run: EmbeddedRunCeremonyStepRequest::try_from(value)?,
            reservation: embedded_budget_fields::reservation(object)?,
        })
    }
}
