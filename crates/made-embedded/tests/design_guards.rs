use made_app::usecases::{
    CeremonyDesignDocument, CeremonyDesignExitGuard, CeremonyDesignOutputFieldGuard,
    CeremonyDesignParticipant, CeremonyDesignRepeat, CeremonyDesignStage,
    CeremonyDesignStepRepeatExhaustedGuard,
};
use made_core::value_objects::{
    CeremonyDescription, CeremonyName, GuardCondition, OutputName, RoleId, Rounds, StepId,
    StepInstructions, StepIteration, StepOutputField,
};
use made_embedded::EmbeddedMade;
use serde_json::json;

fn stage(id: &str, owner: &RoleId, instructions: &str) -> CeremonyDesignStage {
    CeremonyDesignStage::new(
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
    let review = CeremonyDesignStage::new(
        StepId::new("review").unwrap(),
        owner.clone(),
        StepInstructions::new("Review the candidate.").unwrap(),
        None,
        None,
        None,
        Rounds::new(0).unwrap(),
        Some(CeremonyDesignRepeat::new(
            StepIteration::new(3).unwrap(),
            StepOutputField::new("accepted").unwrap(),
            json!(true),
        )),
    )
    .with_exit_guards(vec![
        CeremonyDesignExitGuard::OutputField(CeremonyDesignOutputFieldGuard::new(
            StepId::new("produce").unwrap(),
            StepOutputField::new("candidate").unwrap(),
            json!({"quality": "ready"}),
        )),
        CeremonyDesignExitGuard::StepRepeatExhausted(CeremonyDesignStepRepeatExhaustedGuard::new(
            StepId::new("review").unwrap(),
        )),
    ]);
    let document = CeremonyDesignDocument::new(
        CeremonyName::new("facade_guard_design").unwrap(),
        None,
        CeremonyDescription::new("Produce and review one candidate.").unwrap(),
        Vec::new(),
        Vec::new(),
        vec![OutputName::new("candidate").unwrap()],
        vec![CeremonyDesignParticipant::new(owner, Vec::new())],
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
