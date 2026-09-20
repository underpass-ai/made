//! Which compositions actually block which, once bounded loops are
//! taken into account.
//!
//! A system that sends work back for revision has a cycle in its
//! dependencies, and read literally a cycle means nothing can ever
//! start. The bounded loop is what says which edge is the way back:
//! `Loop { after }` on a composition declares that it runs after that
//! one, so the dependency pointing the other way — that one waiting
//! for this one — is the back edge, and it is cut for the purpose of
//! deciding what can begin.
//!
//! Cut only here. The full graph, back edges and all, is what the
//! analysis reasons about when it asks whether a cycle is bounded.

use std::collections::{BTreeMap, BTreeSet};

use crate::value_objects::{CeremonyComposition, SystemCeremonyId};

/// Every dependency each composition must wait for before it can run.
pub(super) fn blocking(
    ceremonies: &BTreeMap<SystemCeremonyId, CeremonyComposition>,
) -> BTreeMap<SystemCeremonyId, Vec<SystemCeremonyId>> {
    let back = back_edges(ceremonies);
    ceremonies
        .iter()
        .map(|(id, composition)| {
            let waits = composition
                .depends_on()
                .iter()
                .filter(|dependency| !back.contains(&(id.clone(), (*dependency).clone())))
                .cloned()
                .collect();
            (id.clone(), waits)
        })
        .collect()
}

/// The edges a round travels back along: for each bounded loop, the
/// dependency of its anchor on the looping composition.
fn back_edges(
    ceremonies: &BTreeMap<SystemCeremonyId, CeremonyComposition>,
) -> BTreeSet<(SystemCeremonyId, SystemCeremonyId)> {
    ceremonies
        .iter()
        .filter_map(|(id, composition)| {
            composition
                .activation()
                .loops_after()
                .map(|anchor| (anchor.clone(), id.clone()))
        })
        .collect()
}
