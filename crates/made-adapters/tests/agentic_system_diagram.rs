//! The design, drawn — and the same thing said in sentences.

use made_adapters::mermaid::AgenticSystemMermaidDiagram;
use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::ports::AgenticSystemDiagramPort;
use made_core::value_objects::{
    AgenticSystemExecutionId, AgenticSystemId, AttentionPolicy, Capability, CeremonyActivation,
    CeremonyComposition, CeremonyDefinitionDigest, CeremonyExecutionLink, CeremonyId, CeremonyName,
    CeremonyVersion, ChannelName, CollaborationKind, CollaborationLink, DefinitionPin,
    IndependenceGroup, LinkStatus, LogicalParticipant, LoopRound, LoopRounds,
    ParticipantBindingPolicy, ParticipantId, ParticipantKind, Responsibility, SupervisionPolicy,
    SystemCeremonyId, SystemPurpose, SystemRole, SystemRoleId, SystemRoleKind,
    UnavailabilityReason,
};
use time::OffsetDateTime;

#[test]
fn the_drawing_shows_the_spine_the_lanes_and_every_edge_style() {
    let diagram = AgenticSystemMermaidDiagram::new()
        .render(&design(), None)
        .expect("a design renders");
    let mermaid = diagram.mermaid();

    assert!(mermaid.starts_with("flowchart LR"));
    for lane in [
        "subgraph lane_users [\"Users\"]",
        "subgraph lane_host [\"Host task\"]",
        "subgraph lane_engine [\"MADE\"]",
        "subgraph lane_architecture [\"Architecture\"]",
        "subgraph lane_ceremonies [\"Ceremonies\"]",
        "subgraph lane_legend [\"Legend\"]",
    ] {
        assert!(mermaid.contains(lane), "missing lane: {lane}\n{mermaid}");
    }
    // Every kind is drawn differently and every style is in the key,
    // so a reader never has to guess what a line means.
    for kind in CollaborationKind::every() {
        assert!(
            mermaid.contains(kind.edge()),
            "missing edge style for {kind}"
        );
        assert!(mermaid.contains(&format!("legend_to_{}", position(kind))));
    }
    assert!(mermaid.contains("p_operator -->|chat| p_writer"));
    assert!(mermaid.contains("p_writer ==> c_drafting"));
    assert!(mermaid.contains("c_drafting -.-> c_review"));
    assert!(mermaid.contains("up to 2 rounds"));
}

/// A design on its own carries no observed state. Marking every
/// ceremony "pending" would be saying something about reality that
/// nobody has looked at yet.
#[test]
fn a_design_with_no_run_claims_nothing_about_what_happened() {
    let diagram = AgenticSystemMermaidDiagram::new()
        .render(&design(), None)
        .expect("a design renders");

    assert!(!diagram.mermaid().contains("class c_drafting"));
    assert!(!diagram
        .text_equivalent()
        .iter()
        .any(|line| line.contains("Observed:")));
}

#[test]
fn a_run_marks_what_was_intended_and_what_was_observed() {
    let system = design();
    let diagram = AgenticSystemMermaidDiagram::new()
        .render(&system, Some(&run(&system)))
        .expect("a run renders");

    assert!(diagram.mermaid().contains("class c_drafting linkCompleted"));
    assert!(diagram.mermaid().contains("class c_review linkSkipped"));
    let text = diagram.text_equivalent().join("\n");
    assert!(text.contains("Observed: completed as instance `c-1`."));
    assert!(text.contains("Observed: skipped."));
    assert!(text.contains("Reason: no independent reviewer is available."));
}

#[test]
fn the_text_equivalent_carries_what_the_drawing_carries() {
    let diagram = AgenticSystemMermaidDiagram::new()
        .render(&design(), None)
        .expect("a design renders");
    let text = diagram.text_equivalent().join("\n");

    assert!(text.contains("System `delivery` at revision 1: keep the promise we made"));
    assert!(text.contains("It is the integrator: the system answers to it."));
    assert!(text.contains("Participant `writer` plays role `author` as an agent"));
    assert!(text.contains("declares `drafting`"));
    assert!(text.contains("in independence group `house-model`"));
    assert!(text.contains("`operator` has a communication relationship with `writer` over `chat`"));
    assert!(text.contains("It loops at most 2 times."));
    assert!(text.contains("Role `reviewer` must be independent of role `author`."));
    assert!(text.contains("Edge styles: `-->` is communication"));
}

fn position(kind: CollaborationKind) -> usize {
    CollaborationKind::every()
        .into_iter()
        .position(|candidate| candidate == kind)
        .expect("every kind is in the list")
}

fn design() -> AgenticSystem {
    let integrator = SystemRoleId::new("integrator").unwrap();
    let author = SystemRoleId::new("author").unwrap();
    let reviewer = SystemRoleId::new("reviewer").unwrap();
    AgenticSystem::draft(
        AgenticSystemId::new("delivery").unwrap(),
        SystemPurpose::new("keep the promise we made").unwrap(),
        integrator.clone(),
        [
            SystemRole::new(
                integrator.clone(),
                Responsibility::new("drives the system from outside it").unwrap(),
                SystemRoleKind::Integrator,
            ),
            SystemRole::new(
                author.clone(),
                Responsibility::new("writes the draft").unwrap(),
                SystemRoleKind::Contributor,
            ),
            SystemRole::new(
                reviewer.clone(),
                Responsibility::new("reads it critically").unwrap(),
                SystemRoleKind::Reviewer,
            ),
        ],
        [
            LogicalParticipant::new(
                ParticipantId::new("operator").unwrap(),
                integrator,
                ParticipantKind::Person,
                ParticipantBindingPolicy::default(),
            ),
            LogicalParticipant::new(
                ParticipantId::new("writer").unwrap(),
                author.clone(),
                ParticipantKind::Agent,
                ParticipantBindingPolicy::new(
                    None,
                    [Capability::new("drafting").unwrap()],
                    Some(IndependenceGroup::new("house-model").unwrap()),
                ),
            ),
            LogicalParticipant::new(
                ParticipantId::new("critic").unwrap(),
                reviewer.clone(),
                ParticipantKind::Agent,
                ParticipantBindingPolicy::default(),
            ),
        ],
        [CollaborationLink::new(
            ParticipantId::new("operator").unwrap(),
            ParticipantId::new("writer").unwrap(),
            CollaborationKind::Communication,
            Some(ChannelName::new("chat").unwrap()),
            false,
        )
        .unwrap()],
        [],
        [
            CeremonyComposition::new(
                SystemCeremonyId::new("drafting").unwrap(),
                pin(0x0a),
                SystemPurpose::new("produce a draft").unwrap(),
                [],
                CeremonyActivation::Manual,
                [(
                    made_core::value_objects::RoleId::new("AUTHOR").unwrap(),
                    ParticipantId::new("writer").unwrap(),
                )],
                [],
            ),
            CeremonyComposition::new(
                SystemCeremonyId::new("review").unwrap(),
                pin(0x0b),
                SystemPurpose::new("read it critically").unwrap(),
                [SystemCeremonyId::new("drafting").unwrap()],
                CeremonyActivation::Loop {
                    after: SystemCeremonyId::new("drafting").unwrap(),
                    max_rounds: LoopRounds::new(2).unwrap(),
                },
                [(
                    made_core::value_objects::RoleId::new("REVIEWER").unwrap(),
                    ParticipantId::new("critic").unwrap(),
                )],
                [],
            ),
        ],
        SupervisionPolicy::new(
            [],
            [made_core::value_objects::IndependenceRule::new(reviewer, author).unwrap()],
            [],
        ),
        AttentionPolicy::default(),
        OffsetDateTime::UNIX_EPOCH,
    )
    .expect("a valid draft")
}

fn run(system: &AgenticSystem) -> AgenticSystemExecution {
    let drafting = SystemCeremonyId::new("drafting").unwrap();
    let review = SystemCeremonyId::new("review").unwrap();
    AgenticSystemExecution::plan(
        AgenticSystemExecutionId::new("run-1").unwrap(),
        system.pin().expect("a design pins"),
        [
            (
                drafting,
                CeremonyExecutionLink::pending(pin(0x0a))
                    .started(CeremonyId::new("c-1").unwrap(), LoopRound::ZERO.next())
                    .settled(LinkStatus::Completed),
            ),
            (
                review,
                CeremonyExecutionLink::pending(pin(0x0b)).skipped(
                    UnavailabilityReason::new("no independent reviewer is available").unwrap(),
                ),
            ),
        ],
        [],
        [],
        OffsetDateTime::UNIX_EPOCH,
    )
    .expect("a valid run")
}

fn pin(digest: u8) -> DefinitionPin {
    DefinitionPin::new(
        CeremonyName::new("delivery").unwrap(),
        CeremonyVersion::new("1.0").unwrap(),
        CeremonyDefinitionDigest::from_bytes([digest; 32]),
    )
}

/// Prints the drawing, so a human can paste it into a renderer and
/// see what the assertions above only describe.
#[test]
fn the_drawing_can_be_read_by_a_person() {
    let system = design();
    let diagram = AgenticSystemMermaidDiagram::new()
        .render(&system, Some(&run(&system)))
        .expect("a run renders");

    println!("{}", diagram.mermaid());
    println!("---");
    println!("{}", diagram.text_equivalent().join("\n"));
    assert!(!diagram.mermaid().is_empty());
}
