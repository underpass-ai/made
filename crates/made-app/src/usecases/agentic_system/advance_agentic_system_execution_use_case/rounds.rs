//! Sending work back round, a bounded number of times.
//!
//! When a looping composition completes and its bound is not yet
//! reached, every member of its cycle is reopened for the next round.
//! Reopening the anchor alone would leave the loop itself settled and
//! the round would never come back to it.

use std::collections::BTreeSet;

use made_core::entities::{AgenticSystem, AgenticSystemExecution};
use made_core::error::DomainError;
use made_core::value_objects::{LinkStatus, SystemCeremonyId};
use time::OffsetDateTime;

pub(super) fn reopen(
    execution: &AgenticSystemExecution,
    system: &AgenticSystem,
    now: OffsetDateTime,
) -> Result<AgenticSystemExecution, DomainError> {
    let mut next = execution.clone();
    for (ceremony, composition) in system.ceremonies() {
        let Some(bound) = composition.activation().max_rounds() else {
            continue;
        };
        let Some(link) = next.link(ceremony) else {
            continue;
        };
        if link.status() != LinkStatus::Completed || link.round().is_exhausted(bound) {
            continue;
        }
        for member in cycle_of(system, ceremony) {
            let Some(member_link) = next.link(&member) else {
                continue;
            };
            next = next.with_link(&member, member_link.reopened(), now)?;
        }
    }
    Ok(next)
}

/// Everything that waits for this composition and that this
/// composition waits for, through declared dependencies.
fn cycle_of(system: &AgenticSystem, ceremony: &SystemCeremonyId) -> BTreeSet<SystemCeremonyId> {
    let mut members: BTreeSet<SystemCeremonyId> = BTreeSet::new();
    members.insert(ceremony.clone());
    let mut grown = true;
    while grown {
        grown = false;
        for (id, composition) in system.ceremonies() {
            if members.contains(id) {
                continue;
            }
            let touches = composition
                .depends_on()
                .iter()
                .any(|dependency| members.contains(dependency))
                || composition
                    .activation()
                    .loops_after()
                    .is_some_and(|anchor| members.contains(anchor));
            let reached = members.iter().any(|member| {
                system.ceremonies().get(member).is_some_and(|other| {
                    other.depends_on().contains(id) || other.activation().loops_after() == Some(id)
                })
            });
            if touches && reached {
                members.insert(id.clone());
                grown = true;
            }
        }
    }
    members
}
