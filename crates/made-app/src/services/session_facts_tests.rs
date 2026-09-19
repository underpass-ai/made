use made_core::value_objects::{
    AuditEventType, CeremonyId, ExecutionOperationId, ExecutionReceiptId, ExecutionReceiptLink,
    ExecutionReceiptLinkKind, StateIteration, StateVisit, StepClaimFence, StepId, StepIteration,
};

use super::{event_id, execution_receipt_about};

fn link(applied: char, kind: ExecutionReceiptLinkKind) -> ExecutionReceiptLink {
    let operation = ExecutionOperationId::for_step(
        &CeremonyId::new("receipt-fact").unwrap(),
        &StepId::new("work").unwrap(),
        StateVisit::FIRST,
        StateIteration::FIRST,
        StepIteration::FIRST,
    );
    let producer = StepClaimFence::new("1".repeat(64)).unwrap();
    ExecutionReceiptLink::new(
        ExecutionReceiptId::for_operation(&operation),
        operation,
        producer.clone(),
        if kind == ExecutionReceiptLinkKind::Direct {
            producer
        } else {
            StepClaimFence::new(applied.to_string().repeat(64)).unwrap()
        },
        kind,
    )
    .unwrap()
}

#[test]
fn direct_receipt_keeps_its_historical_event_identity() {
    let direct = link('1', ExecutionReceiptLinkKind::Direct);
    let about = execution_receipt_about(&direct);

    assert_eq!(about, format!("execution_receipt:{}", direct.receipt_id()));
    assert_eq!(
        event_id(
            &CeremonyId::new("receipt-fact").unwrap(),
            AuditEventType::ExecutionReceiptLinked,
            &about,
        )
        .unwrap()
        .as_str(),
        format!(
            "receipt-fact:execution_receipt_linked:execution_receipt:{}",
            direct.receipt_id()
        )
    );
}

#[test]
fn receipt_adoptions_for_different_fences_have_different_event_ids() {
    let ceremony = CeremonyId::new("receipt-fact").unwrap();
    let first = link('2', ExecutionReceiptLinkKind::Adopted);
    let second = link('3', ExecutionReceiptLinkKind::Adopted);
    let first_id = event_id(
        &ceremony,
        AuditEventType::ExecutionReceiptLinked,
        &execution_receipt_about(&first),
    )
    .unwrap();
    let second_id = event_id(
        &ceremony,
        AuditEventType::ExecutionReceiptLinked,
        &execution_receipt_about(&second),
    )
    .unwrap();

    assert_ne!(first_id, second_id);
    assert!(first_id
        .as_str()
        .ends_with(first.applied_claim_fence().as_str()));
    assert!(second_id
        .as_str()
        .ends_with(second.applied_claim_fence().as_str()));
}
