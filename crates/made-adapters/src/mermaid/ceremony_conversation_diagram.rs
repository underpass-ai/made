use std::collections::BTreeSet;

use made_core::entities::CeremonyDefinition;
use made_core::value_objects::{RoleAction, RoleId, StateId, StepId};

#[derive(Debug, Default, Clone, Copy)]
pub struct CeremonyConversationDiagram;

impl CeremonyConversationDiagram {
    #[must_use]
    pub fn render(definition: &CeremonyDefinition) -> String {
        let participants = participants(definition);
        let planned_steps = planned_steps(definition);
        let all_participants = participant_range(&participants);
        let mut lines = vec![
            "sequenceDiagram".to_owned(),
            format!(
                "  title {} v{}",
                escape_label(definition.name().as_str()),
                escape_label(definition.version().as_str())
            ),
        ];

        for (index, role_id) in &participants {
            lines.push(format!(
                "  participant R{} as {}",
                index,
                escape_label(role_id.as_str())
            ));
        }

        for (position, (state_id, step_id, actor_index)) in planned_steps.iter().enumerate() {
            lines.push(format!(
                "  Note over {}: State {}",
                all_participants,
                escape_label(state_id.as_str())
            ));
            let target_index = planned_steps
                .get(position + 1)
                .map_or(*actor_index, |(_, _, next_actor)| *next_actor);
            let step = definition
                .step(step_id)
                .expect("planned step comes from ceremony definition");
            lines.push(format!(
                "  R{}->>R{}: {} [{}]",
                actor_index,
                target_index,
                escape_label(step_id.as_str()),
                escape_label(step.handler_kind().as_str())
            ));

            for transition in definition.available_transitions(state_id) {
                let guards = transition
                    .required_guards()
                    .iter()
                    .map(|guard| escape_label(guard.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let guard_suffix = if guards.is_empty() {
                    String::new()
                } else {
                    format!(" (guards: {guards})")
                };
                lines.push(format!(
                    "  Note over {}: {} -> {}{}",
                    all_participants,
                    escape_label(transition.trigger().as_str()),
                    escape_label(transition.to().as_str()),
                    guard_suffix
                ));
            }
        }

        lines.join("\n")
    }
}

fn participants(definition: &CeremonyDefinition) -> Vec<(usize, RoleId)> {
    let roles = definition
        .roles()
        .keys()
        .cloned()
        .enumerate()
        .map(|(index, role_id)| (index + 1, role_id))
        .collect::<Vec<_>>();
    if roles.is_empty() {
        vec![(1, RoleId::new("SYSTEM").expect("static role id is valid"))]
    } else {
        roles
    }
}

fn participant_range(participants: &[(usize, RoleId)]) -> String {
    let first = participants
        .first()
        .map(|(index, _)| format!("R{index}"))
        .expect("participants are never empty");
    let last = participants
        .last()
        .map(|(index, _)| format!("R{index}"))
        .expect("participants are never empty");
    if first == last {
        first
    } else {
        format!("{first},{last}")
    }
}

fn planned_steps(definition: &CeremonyDefinition) -> Vec<(StateId, StepId, usize)> {
    let participants = participants(definition);
    ordered_state_ids(definition)
        .into_iter()
        .flat_map(|state_id| {
            let participants = participants.clone();
            definition
                .steps_for_state(&state_id)
                .map(move |step| {
                    let actor_index =
                        actor_index_for_step(definition, &participants, step.id()).unwrap_or(1);
                    (state_id.clone(), step.id().clone(), actor_index)
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn ordered_state_ids(definition: &CeremonyDefinition) -> Vec<StateId> {
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = definition.initial_state_id().clone();

    loop {
        if !seen.insert(current.clone()) {
            break;
        }
        ordered.push(current.clone());
        if definition.is_terminal_state(&current) {
            break;
        }
        let Some(next) = definition
            .available_transitions(&current)
            .next()
            .map(|transition| transition.to().clone())
        else {
            break;
        };
        current = next;
    }

    for state_id in definition.states().keys() {
        if !seen.contains(state_id) {
            ordered.push(state_id.clone());
        }
    }

    ordered
}

fn actor_index_for_step(
    definition: &CeremonyDefinition,
    participants: &[(usize, RoleId)],
    step_id: &StepId,
) -> Option<usize> {
    let action = RoleAction::step(step_id.clone());
    participants
        .iter()
        .find(|(_, role_id)| definition.role_allows(role_id, &action))
        .map(|(index, _)| *index)
}

fn escape_label(raw: &str) -> String {
    raw.replace(['\n', '\r', '"'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml::CeremonyDefinitionYaml;

    const CEREMONY: &str = r#"
version: "1.0"
name: "editorial_planning_meeting"
description: "Non-technical editorial planning ceremony"
states:
  - id: OPENING
    initial: true
  - id: DIVERGING
  - id: REVIEWING
  - id: SYNTHESIZING
  - id: CLOSED
    terminal: true
transitions:
  - from: OPENING
    to: DIVERGING
    trigger: context_shared
    guards:
      - open_room_completed
  - from: DIVERGING
    to: REVIEWING
    trigger: options_collected
    guards:
      - customer_story_completed
  - from: REVIEWING
    to: SYNTHESIZING
    trigger: risks_reviewed
    guards:
      - risk_check_completed
  - from: SYNTHESIZING
    to: CLOSED
    trigger: decision_written
    guards:
      - decision_summary_completed
steps:
  - id: open_room
    state: OPENING
    handler: facilitation_prompt
  - id: customer_story
    state: DIVERGING
    handler: persona_prompt
  - id: risk_check
    state: REVIEWING
    handler: challenge_prompt
  - id: decision_summary
    state: SYNTHESIZING
    handler: synthesis_prompt
guards:
  open_room_completed:
    type: automated
    check: "step_status:open_room:COMPLETED"
  customer_story_completed:
    type: automated
    check: "step_status:customer_story:COMPLETED"
  risk_check_completed:
    type: automated
    check: "step_status:risk_check:COMPLETED"
  decision_summary_completed:
    type: automated
    check: "step_status:decision_summary:COMPLETED"
roles:
  - id: FACILITATOR
    allowed_actions:
      - open_room
      - context_shared
  - id: CUSTOMER_ADVOCATE
    allowed_actions:
      - customer_story
      - options_collected
  - id: RISK_REVIEWER
    allowed_actions:
      - risk_check
      - risks_reviewed
  - id: SYNTHESIZER
    allowed_actions:
      - decision_summary
      - decision_written
"#;

    #[test]
    fn renders_four_role_ceremony_as_mermaid_sequence() {
        let definition = CeremonyDefinitionYaml::parse_str(CEREMONY).unwrap();

        let diagram = CeremonyConversationDiagram::render(&definition);

        assert!(diagram.starts_with("sequenceDiagram\n"));
        assert!(diagram.contains("  participant R1 as CUSTOMER_ADVOCATE"));
        assert!(diagram.contains("  participant R2 as FACILITATOR"));
        assert!(diagram.contains("  participant R3 as RISK_REVIEWER"));
        assert!(diagram.contains("  participant R4 as SYNTHESIZER"));
        assert!(diagram.contains("  R2->>R1: open_room [facilitation_prompt]"));
        assert!(diagram.contains("  R1->>R3: customer_story [persona_prompt]"));
        assert!(diagram.contains("  R3->>R4: risk_check [challenge_prompt]"));
        assert!(diagram.contains("  R4->>R4: decision_summary [synthesis_prompt]"));
        assert!(diagram.contains("context_shared -> DIVERGING"));
        assert!(diagram.contains("decision_written -> CLOSED"));
    }
}
