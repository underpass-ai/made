use crate::entities::ceremony_commands::{ApplyExecutionReceiptResult, ApplyStepResult};
use crate::entities::ceremony_events::ExecutionReceiptLinked;
use crate::entities::{CeremonyDefinition, CeremonyEvent, CeremonyInstance};
use crate::error::DomainError;
use crate::value_objects::{ExecutionOperationId, ExecutionReceiptLinkKind};

impl CeremonyInstance {
    pub(super) fn decide_apply_execution_receipt_result(
        &self,
        command: &ApplyExecutionReceiptResult,
        definition: &CeremonyDefinition,
    ) -> Result<Vec<CeremonyEvent>, DomainError> {
        command.receipt_link.validate()?;
        if command.receipt_link.applied_claim_fence() != &command.claim_fence {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt link does not target the completing claim fence",
            });
        }
        let original = self
            .execution_receipt_links
            .get(command.receipt_link.operation_id());
        if original == Some(&command.receipt_link) {
            return Ok(Vec::new());
        }
        if command.receipt_link.kind() == ExecutionReceiptLinkKind::Direct && original.is_some() {
            return Err(DomainError::Conflict {
                what: "execution_receipt_link",
            });
        }
        if command.receipt_link.kind() == ExecutionReceiptLinkKind::Adopted {
            if let Some(stored) = self.execution_receipt_adoption(
                command.receipt_link.operation_id(),
                command.receipt_link.applied_claim_fence(),
            ) {
                return if stored == &command.receipt_link {
                    Ok(Vec::new())
                } else {
                    Err(DomainError::Conflict {
                        what: "execution_receipt_adoption",
                    })
                };
            }
            self.require_admits_new_work("adopt_execution_receipt")?;
            // The durable receipt may precede every ceremony link when its
            // producer dies before completing the claim. In that case this
            // adoption is the first observation; do not invent a direct one.
            if let Some(original) = original {
                require_same_receipt(original, &command.receipt_link)?;
            }
        }

        let record = self
            .step_records
            .get(&command.step_id)
            .ok_or(DomainError::NotFound {
                what: "ceremony_instance.step_record",
            })?;
        let coordinates = self
            .retired_deadline_claims
            .get(&command.claim_fence)
            .filter(|deadline| deadline.step_id() == &command.step_id)
            .map_or(
                (
                    record.state_visit(),
                    record.state_iteration(),
                    record.iteration(),
                ),
                |deadline| {
                    (
                        deadline.state_visit(),
                        deadline.state_iteration(),
                        deadline.step_iteration(),
                    )
                },
            );
        let expected_operation_id = ExecutionOperationId::for_step(
            &self.id,
            &command.step_id,
            coordinates.0,
            coordinates.1,
            coordinates.2,
        );
        if command.receipt_link.operation_id() != &expected_operation_id {
            return Err(DomainError::InvariantViolated {
                reason: "execution receipt operation does not match the step coordinates",
            });
        }

        let mut result_events = self.decide_apply_step_result(
            &ApplyStepResult {
                step_id: command.step_id.clone(),
                claim_fence: command.claim_fence.clone(),
                result: command.result.clone(),
                now: command.now,
            },
            definition,
        )?;
        let mut events = Vec::with_capacity(result_events.len() + 1);
        events.push(CeremonyEvent::ExecutionReceiptLinked(
            ExecutionReceiptLinked {
                step_id: command.step_id.clone(),
                link: command.receipt_link.clone(),
                linked_at: command.now,
            },
        ));
        events.append(&mut result_events);
        Ok(events)
    }
}

fn require_same_receipt(
    original: &crate::value_objects::ExecutionReceiptLink,
    adoption: &crate::value_objects::ExecutionReceiptLink,
) -> Result<(), DomainError> {
    if original.receipt_id() != adoption.receipt_id()
        || original.operation_id() != adoption.operation_id()
        || original.producer_claim_fence() != adoption.producer_claim_fence()
    {
        return Err(DomainError::Conflict {
            what: "execution_receipt_adoption",
        });
    }
    Ok(())
}
