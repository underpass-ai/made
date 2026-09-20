//! The design drawn as one left-to-right flow.
//!
//! The lanes are the path a piece of work actually takes: somebody
//! asks, a host task picks it up, the integrator drives it, MADE
//! coordinates, the architecture says who does what, and the
//! ceremonies are where the work happens. Drawing participants without
//! that spine produces a correct graph nobody can read.

use std::collections::BTreeMap;

use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::value_objects::{
    CollaborationKind, LinkStatus, LogicalParticipant, ParticipantId, ParticipantKind,
    SystemCeremonyId,
};

/// Fixed nodes: the spine every design shares, whatever it composes.
const USERS: &str = "users";
const HOST: &str = "host";
const ENGINE: &str = "engine";

pub(super) fn render(system: &AgenticSystem, execution: Option<&AgenticSystemExecution>) -> String {
    // Every lane first, then every edge. Mermaid puts a node in the
    // subgraph that declares it, and a node first seen in an edge is
    // already in the top-level graph by the time its lane opens — so
    // an edge written early quietly empties the lane it belonged to.
    let mut lines = vec!["flowchart LR".to_owned()];
    lines.extend(lanes());
    lines.extend(architecture(system));
    lines.extend(ceremonies(system, execution));
    lines.extend(legend());
    lines.extend(spine(system));
    lines.extend(collaboration(system));
    lines.extend(execution_edges(system));
    lines.extend(classes());
    lines.extend(statuses(system, execution));
    lines.join("\n")
}

/// Users, the host task and the engine: the lanes every design shares.
fn lanes() -> Vec<String> {
    vec![
        "  subgraph lane_users [\"Users\"]".to_owned(),
        format!("    {USERS}[\"whoever asked for this\"]"),
        "  end".to_owned(),
        "  subgraph lane_host [\"Host task\"]".to_owned(),
        format!("    {HOST}[\"the agent host running the integrator\"]"),
        "  end".to_owned(),
        "  subgraph lane_engine [\"MADE\"]".to_owned(),
        format!("    {ENGINE}[\"MADE coordinates and records\"]"),
        "  end".to_owned(),
    ]
}

/// Where the work comes from and what carries it: a user asks, a host
/// task picks it up, the integrator drives it and MADE records it.
fn spine(system: &AgenticSystem) -> Vec<String> {
    let integrator = system
        .participants()
        .values()
        .find(|participant| participant.role() == system.integrator())
        .map(LogicalParticipant::id);
    let mut lines = vec![format!("  {USERS} --> {HOST}")];
    if let Some(integrator) = integrator {
        lines.push(format!("  {HOST} --> {}", node(integrator)));
        lines.push(format!("  {} --> {ENGINE}", node(integrator)));
    } else {
        lines.push(format!("  {HOST} --> {ENGINE}"));
    }
    lines
}

/// Who takes part, grouped by whether the integrator is one of them.
fn architecture(system: &AgenticSystem) -> Vec<String> {
    let mut lines = vec!["  subgraph lane_architecture [\"Architecture\"]".to_owned()];
    for participant in system.participants().values() {
        let role = participant.role();
        let kind = match participant.kind() {
            ParticipantKind::Person => "person",
            ParticipantKind::Agent => "agent",
        };
        let marker = if role == system.integrator() {
            " · drives the system"
        } else {
            ""
        };
        lines.push(format!(
            "    {}[\"{} — {} ({kind}){marker}\"]",
            node(participant.id()),
            escape(participant.id().as_str()),
            escape(role.as_str()),
        ));
    }
    lines.push("  end".to_owned());
    lines
}

/// What the system composes, and — when a run is being shown — what
/// became of each composition.
fn ceremonies(system: &AgenticSystem, execution: Option<&AgenticSystemExecution>) -> Vec<String> {
    let observed = observed_statuses(execution);
    let mut lines = vec!["  subgraph lane_ceremonies [\"Ceremonies\"]".to_owned()];
    for (id, composition) in system.ceremonies() {
        let rounds = composition
            .activation()
            .max_rounds()
            .map(|rounds| format!(" · up to {rounds} rounds"))
            .unwrap_or_default();
        let status = observed
            .get(id)
            .map(|status| format!(" · {status}"))
            .unwrap_or_default();
        lines.push(format!(
            "    {}[\"{} — {}{rounds}{status}\"]",
            ceremony_node(id),
            escape(id.as_str()),
            escape(&composition.pin().to_string()),
        ));
    }
    lines.push("  end".to_owned());
    lines
}

/// The topology: one edge per declared collaboration, drawn in the
/// style its kind calls for.
fn collaboration(system: &AgenticSystem) -> Vec<String> {
    system
        .topology()
        .iter()
        .map(|link| {
            // A handoff belongs inside the label rather than beside
            // the edge: anything after the target node ends the
            // statement, and the rest of the diagram with it.
            let mut label: Vec<String> = Vec::new();
            label.extend(link.channel().map(|channel| escape(channel.as_str())));
            if link.is_handoff() {
                label.push("handoff".to_owned());
            }
            let label = if label.is_empty() {
                String::new()
            } else {
                format!("|{}|", label.join(" · "))
            };
            format!(
                "  {} {}{label} {}",
                node(link.from()),
                link.kind().edge(),
                node(link.to()),
            )
        })
        .collect()
}

/// Who sits in which ceremony, from the seating the design declares.
fn execution_edges(system: &AgenticSystem) -> Vec<String> {
    let mut lines = Vec::new();
    for (id, composition) in system.ceremonies() {
        let mut seated: Vec<&ParticipantId> = composition.role_bindings().values().collect();
        seated.sort_unstable();
        seated.dedup();
        for participant in seated {
            lines.push(format!("  {} ==> {}", node(participant), ceremony_node(id)));
        }
        for predecessor in composition.predecessors() {
            if system.ceremonies().contains_key(predecessor) {
                lines.push(format!(
                    "  {} -.-> {}",
                    ceremony_node(predecessor),
                    ceremony_node(id)
                ));
            }
        }
    }
    lines
}

/// What each edge style means, said on the picture.
///
/// A diagram whose key lives in the prose beside it is a diagram that
/// arrives without its key the first time somebody pastes it
/// somewhere else.
fn legend() -> Vec<String> {
    let mut lines = vec!["  subgraph lane_legend [\"Legend\"]".to_owned()];
    for (index, kind) in CollaborationKind::every().into_iter().enumerate() {
        lines.push(format!("    legend_from_{index}[\" \"]"));
        lines.push(format!("    legend_to_{index}[\"{kind}\"]"));
        lines.push(format!(
            "    legend_from_{index} {} legend_to_{index}",
            kind.edge()
        ));
    }
    lines.push("  end".to_owned());
    lines
}

fn classes() -> Vec<String> {
    vec![
        "  classDef linkPending stroke-dasharray: 4 4".to_owned(),
        "  classDef linkStarted stroke-width: 2px".to_owned(),
        "  classDef linkCompleted stroke-width: 3px".to_owned(),
        "  classDef linkFailed stroke-width: 3px,stroke-dasharray: 2 2".to_owned(),
        "  classDef linkSkipped stroke-dasharray: 1 6".to_owned(),
    ]
}

/// What a run actually did, as a class on each composition.
///
/// Only when a run is being shown. A design on its own has no observed
/// state, and giving every node the "pending" class would say
/// something about reality that nobody has looked at.
fn statuses(system: &AgenticSystem, execution: Option<&AgenticSystemExecution>) -> Vec<String> {
    let observed = observed_statuses(execution);
    system
        .ceremonies()
        .keys()
        .filter_map(|id| {
            observed
                .get(id)
                .map(|status| format!("  class {} {}", ceremony_node(id), status.diagram_class()))
        })
        .collect()
}

fn observed_statuses(
    execution: Option<&AgenticSystemExecution>,
) -> BTreeMap<&SystemCeremonyId, LinkStatus> {
    execution
        .map(|execution| {
            execution
                .ceremonies()
                .iter()
                .map(|(id, link)| (id, link.status()))
                .collect()
        })
        .unwrap_or_default()
}

fn node(participant: &ParticipantId) -> String {
    format!("p_{}", sanitize(participant.as_str()))
}

fn ceremony_node(ceremony: &SystemCeremonyId) -> String {
    format!("c_{}", sanitize(ceremony.as_str()))
}

/// Identifiers admit hyphens; Mermaid node ids do not.
fn sanitize(raw: &str) -> String {
    raw.chars()
        .map(|character| if character == '-' { '_' } else { character })
        .collect()
}

/// Quotes and brackets end a label early, and a label that ends early
/// takes the rest of the diagram with it.
fn escape(raw: &str) -> String {
    raw.replace('"', "'").replace(['[', ']', '{', '}'], "")
}
