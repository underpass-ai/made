use made_app::usecases::{
    ProtocolDesignDocument, ProtocolDesignExitGuard, ProtocolDesignOutputFieldGuard,
    ProtocolDesignParticipant, ProtocolDesignRepeat, ProtocolDesignStage,
    ProtocolDesignStepRepeatExhaustedGuard,
};
use made_core::value_objects::{
    GuardCondition, OutputName, ProtocolDescription, ProtocolName, RoleId, Rounds, StepId,
    StepInstructions, StepIteration, StepOutputField,
};
use made_embedded::EmbeddedMade;
use serde_json::json;

fn stage(id: &str, owner: &RoleId, instructions: &str) -> ProtocolDesignStage {
    ProtocolDesignStage::new(
        StepId::new(id).unwrap(),
        owner.clone(),
        StepInstructions::new(instructions).unwrap(),
        None,
        None,
        None,
        Rounds::new(0).unwrap(),
        None,
    )
}

#[test]
fn facade_designs_typed_output_and_exhausted_guards() {
    let owner = RoleId::new("FACILITATOR").unwrap();
    let produce = stage("produce", &owner, "Produce a candidate.");
    let review = ProtocolDesignStage::new(
        StepId::new("review").unwrap(),
        owner.clone(),
        StepInstructions::new("Review the candidate.").unwrap(),
        None,
        None,
        None,
        Rounds::new(0).unwrap(),
        Some(ProtocolDesignRepeat::new(
            StepIteration::new(3).unwrap(),
            StepOutputField::new("accepted").unwrap(),
            json!(true),
        )),
    )
    .with_exit_guards(vec![
        ProtocolDesignExitGuard::OutputField(ProtocolDesignOutputFieldGuard::new(
            StepId::new("produce").unwrap(),
            StepOutputField::new("candidate").unwrap(),
            json!({"quality": "ready"}),
        )),
        ProtocolDesignExitGuard::StepRepeatExhausted(ProtocolDesignStepRepeatExhaustedGuard::new(
            StepId::new("review").unwrap(),
        )),
    ]);
    let document = ProtocolDesignDocument::new(
        ProtocolName::new("facade_guard_design").unwrap(),
        None,
        ProtocolDescription::new("Produce and review one candidate.").unwrap(),
        Vec::new(),
        Vec::new(),
        vec![OutputName::new("candidate").unwrap()],
        vec![ProtocolDesignParticipant::new(owner, Vec::new())],
        vec![produce, review],
        None,
        None,
        None,
        None,
    );

    let designed = EmbeddedMade::default().design(&document).unwrap();
    let guards = designed.definition().guards();

    assert!(guards.iter().any(|guard| matches!(
        guard.condition(),
        GuardCondition::OutputField(condition)
            if condition.step_id() == &StepId::new("produce").unwrap()
    )));
    assert!(guards.iter().any(|guard| matches!(
        guard.condition(),
        GuardCondition::StepRepeatExhausted(condition)
            if condition.step_id() == &StepId::new("review").unwrap()
    )));
}
