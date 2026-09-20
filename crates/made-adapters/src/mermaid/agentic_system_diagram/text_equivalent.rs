//! The same picture, said in sentences.
//!
//! Not a fallback. A reader using a screen reader, a reader in a
//! terminal and a reader reviewing a diff all get the same content
//! here as the drawing carries, and it is generated from the same
//! design so the two cannot say different things.

use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::value_objects::{CollaborationKind, ParticipantKind};

pub(super) fn render(
    system: &AgenticSystem,
    execution: Option<&AgenticSystemExecution>,
) -> Vec<String> {
    let mut lines = vec![format!(
        "System `{}` at revision {}: {}",
        system.id(),
        system.revision(),
        system.purpose()
    )];
    lines.extend(roles(system));
    lines.extend(participants(system));
    lines.extend(topology(system));
    lines.extend(compositions(system, execution));
    lines.extend(supervision(system));
    lines.push(legend());
    lines
}

fn roles(system: &AgenticSystem) -> Vec<String> {
    system
        .roles()
        .values()
        .map(|role| {
            let integrator = if role.id() == system.integrator() {
                " It is the integrator: the system answers to it."
            } else {
                ""
            };
            format!(
                "Role `{}` ({}) is answerable for: {}.{integrator}",
                role.id(),
                role.kind(),
                role.responsibility()
            )
        })
        .collect()
}

fn participants(system: &AgenticSystem) -> Vec<String> {
    system
        .participants()
        .values()
        .map(|participant| {
            let capabilities = participant.binding().capabilities();
            let declares = if capabilities.is_empty() {
                "declares no capabilities".to_owned()
            } else {
                format!(
                    "declares {}",
                    capabilities
                        .iter()
                        .map(|capability| format!("`{capability}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            let group = participant
                .binding()
                .independence_group()
                .map(|group| format!(", in independence group `{group}`"))
                .unwrap_or_default();
            format!(
                "Participant `{}` plays role `{}` as {} {} and {declares}{group}.",
                participant.id(),
                participant.role(),
                match participant.kind() {
                    ParticipantKind::Agent => "an",
                    ParticipantKind::Person => "a",
                },
                participant.kind()
            )
        })
        .collect()
}

fn topology(system: &AgenticSystem) -> Vec<String> {
    system
        .topology()
        .iter()
        .map(|link| {
            let channel = link
                .channel()
                .map(|channel| format!(" over `{channel}`"))
                .unwrap_or_default();
            let handoff = if link.is_handoff() {
                " The work itself changes hands here."
            } else {
                ""
            };
            format!(
                "`{}` has a {} relationship with `{}`{channel}.{handoff}",
                link.from(),
                link.kind(),
                link.to()
            )
        })
        .collect()
}

fn compositions(system: &AgenticSystem, execution: Option<&AgenticSystemExecution>) -> Vec<String> {
    system
        .ceremonies()
        .iter()
        .map(|(id, composition)| {
            let waits = composition.predecessors();
            let after = if waits.is_empty() {
                "nothing; it can start straight away".to_owned()
            } else {
                waits
                    .iter()
                    .map(|predecessor| format!("`{predecessor}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let rounds = composition
                .activation()
                .max_rounds()
                .map(|rounds| format!(" It loops at most {rounds} times."))
                .unwrap_or_default();
            let observed = execution
                .and_then(|execution| execution.link(id))
                .map(|link| {
                    let instance = link
                        .instance_id()
                        .map(|instance| format!(" as instance `{instance}`"))
                        .unwrap_or_default();
                    let because = link
                        .skipped_because()
                        .map(|reason| format!(" Reason: {reason}."))
                        .unwrap_or_default();
                    format!(" Observed: {}{instance}.{because}", link.status())
                })
                .unwrap_or_default();
            format!(
                "Ceremony `{id}` runs `{}` to {}. It waits for {after}.{rounds}{observed}",
                composition.pin(),
                composition.purpose()
            )
        })
        .collect()
}

fn supervision(system: &AgenticSystem) -> Vec<String> {
    let supervision = system.supervision();
    let mut lines = Vec::new();
    for approval in supervision.human_approvals() {
        lines.push(format!(
            "Guard `{}` of ceremony `{}` must be answered by a human.",
            approval.guard_name(),
            approval.ceremony()
        ));
    }
    for rule in supervision.independence() {
        lines.push(format!(
            "Role `{}` must be independent of role `{}`.",
            rule.reviewer(),
            rule.reviewed()
        ));
    }
    if system.participants().values().next().is_none() {
        lines.push("No participant is declared.".to_owned());
    }
    lines
}

/// What each edge style in the drawing means.
fn legend() -> String {
    format!(
        "Edge styles: {}.",
        CollaborationKind::every()
            .into_iter()
            .map(|kind| format!("`{}` is {kind}", kind.edge()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
