use made_core::entities::ProtocolEvent;

/// A handoff is keyed on the plan, so re-sealing the same plan derives
/// the same fact and lands once. What a successor was given is keyed
/// on nothing else: a successor is opened once, and the one batch that
/// opens it carries both facts, so rebuilding that batch after a crash
/// derives the same two ids.
pub(super) fn succession_about(event: &ProtocolEvent) -> String {
    match event {
        ProtocolEvent::SuccessorPlanned(planned) => {
            format!("succession:{}", planned.plan.plan_id().as_str())
        }
        ProtocolEvent::SuccessionCarried(_) => "succession:carried".to_owned(),
        _ => unreachable!("only succession events are delegated here"),
    }
}
