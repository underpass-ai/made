//! Can this system ever finish?
//!
//! Work going round for revision is a system working as designed; work
//! going round forever is a system nobody can wait for. The difference
//! is a declared bound, so a cycle is admitted exactly when one of its
//! members is a `Loop` that closes it and says how many rounds.

use std::collections::{BTreeMap, BTreeSet};

use crate::entities::AgenticSystemParts;
use crate::value_objects::{
    AgenticSystemValidationFinding, AgenticSystemValidationLocus, CeremonyComposition,
    SystemCeremonyId,
};

pub(super) fn collect(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    named_predecessors_exist(parts, findings);
    nothing_waits_for_itself(parts, findings);
    let reaches = reachability(parts.ceremonies);
    every_cycle_is_bounded(parts, &reaches, findings);
    every_loop_closes_its_own_cycle(parts, &reaches, findings);
    something_can_start(parts, findings);
}

fn named_predecessors_exist(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for (id, composition) in parts.ceremonies {
        for predecessor in composition.predecessors() {
            if !parts.ceremonies.contains_key(predecessor) {
                findings.push(AgenticSystemValidationFinding::refusal(
                    AgenticSystemValidationLocus::ceremony(id.clone()),
                    format!("ceremony `{predecessor}` is not composed by this system"),
                ));
            }
        }
    }
}

fn nothing_waits_for_itself(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for (id, composition) in parts.ceremonies {
        if composition.depends_on().contains(id) {
            findings.push(AgenticSystemValidationFinding::refusal(
                AgenticSystemValidationLocus::ceremony(id.clone()),
                format!("ceremony `{id}` depends on itself"),
            ));
        }
    }
}

/// Which compositions each composition can reach by waiting.
///
/// Transitive closure rather than a graph walk with a stack: a design
/// holds a handful of compositions, and a matrix answers both
/// questions this module asks — "is this in a cycle" and "are these
/// two in the same one" — without either being a second traversal to
/// keep correct.
fn reachability(
    ceremonies: &BTreeMap<SystemCeremonyId, CeremonyComposition>,
) -> BTreeMap<&SystemCeremonyId, BTreeSet<&SystemCeremonyId>> {
    let mut reaches: BTreeMap<&SystemCeremonyId, BTreeSet<&SystemCeremonyId>> = ceremonies
        .iter()
        .map(|(id, composition)| {
            let direct = composition
                .predecessors()
                .into_iter()
                .filter(|predecessor| ceremonies.contains_key(*predecessor))
                .collect();
            (id, direct)
        })
        .collect();
    let ids: Vec<&SystemCeremonyId> = ceremonies.keys().collect();
    for _ in 0..ids.len() {
        let mut grown = false;
        for id in &ids {
            let expanded: BTreeSet<&SystemCeremonyId> = reaches[id]
                .iter()
                .flat_map(|step| reaches.get(step).into_iter().flatten().copied())
                .collect();
            let entry = reaches.get_mut(id).expect("every id was seeded");
            let before = entry.len();
            entry.extend(expanded);
            grown |= entry.len() != before;
        }
        if !grown {
            break;
        }
    }
    reaches
}

fn every_cycle_is_bounded(
    parts: AgenticSystemParts<'_>,
    reaches: &BTreeMap<&SystemCeremonyId, BTreeSet<&SystemCeremonyId>>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    let mut reported: BTreeSet<&SystemCeremonyId> = BTreeSet::new();
    for (id, composition) in parts.ceremonies {
        if reported.contains(id) || !in_a_cycle(reaches, id) {
            continue;
        }
        let cycle = cycle_of(reaches, id);
        reported.extend(cycle.iter().copied());
        let bounded = cycle.iter().any(|member| {
            parts
                .ceremonies
                .get(*member)
                .is_some_and(|member| member.activation().max_rounds().is_some())
        });
        if bounded {
            continue;
        }
        let _ = composition;
        let named = cycle
            .iter()
            .map(|member| format!("`{member}`"))
            .collect::<Vec<_>>()
            .join(", ");
        findings.push(AgenticSystemValidationFinding::refusal(
            AgenticSystemValidationLocus::ceremony((*id).clone()),
            format!("ceremonies {named} wait for each other and no member declares a bounded loop"),
        ));
    }
}

/// A loop whose anchor it cannot reach is not a loop at all: it is a
/// dependency written in looping clothes, and nothing would ever send
/// work back round it.
fn every_loop_closes_its_own_cycle(
    parts: AgenticSystemParts<'_>,
    reaches: &BTreeMap<&SystemCeremonyId, BTreeSet<&SystemCeremonyId>>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    for (id, composition) in parts.ceremonies {
        let Some(after) = composition.activation().loops_after() else {
            continue;
        };
        if !parts.ceremonies.contains_key(after) {
            continue;
        }
        if cycle_of(reaches, id).contains(after) {
            continue;
        }
        findings.push(AgenticSystemValidationFinding::refusal(
            AgenticSystemValidationLocus::ceremony(id.clone()),
            format!(
                "ceremony `{id}` loops after `{after}`, which is not in a cycle with it, so no \
                 round could ever be sent back"
            ),
        ));
    }
}

fn something_can_start(
    parts: AgenticSystemParts<'_>,
    findings: &mut Vec<AgenticSystemValidationFinding>,
) {
    if parts.ceremonies.is_empty() {
        return;
    }
    // Asked of the blocking graph, not the full one: a bounded loop's
    // way back is not something the first round waits for, and
    // counting it would call every revision loop unstartable.
    let blocking = crate::entities::AgenticSystemParts::blocking_dependencies(&parts);
    let startable = blocking.values().any(Vec::is_empty);
    if !startable {
        findings.push(AgenticSystemValidationFinding::refusal(
            AgenticSystemValidationLocus::System,
            "every composed ceremony waits for another, so a run has nothing to begin with",
        ));
    }
}

fn in_a_cycle(
    reaches: &BTreeMap<&SystemCeremonyId, BTreeSet<&SystemCeremonyId>>,
    id: &SystemCeremonyId,
) -> bool {
    reaches.get(id).is_some_and(|set| set.contains(id))
}

/// Everything that waits for `id` and that `id` waits for: the cycle
/// it belongs to, or just itself when it belongs to none.
fn cycle_of<'design>(
    reaches: &BTreeMap<&'design SystemCeremonyId, BTreeSet<&'design SystemCeremonyId>>,
    id: &SystemCeremonyId,
) -> BTreeSet<&'design SystemCeremonyId> {
    let Some(from_id) = reaches.get(id) else {
        return BTreeSet::new();
    };
    reaches
        .iter()
        .filter(|(other, from_other)| from_id.contains(**other) && from_other.contains(id))
        .map(|(other, _)| *other)
        .collect()
}
