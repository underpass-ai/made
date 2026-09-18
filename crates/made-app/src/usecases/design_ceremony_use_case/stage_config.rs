use std::collections::{BTreeMap, BTreeSet};

use made_core::value_objects::{GuardCondition, StepId};
use serde_json::{json, Value};

use super::{num_agents, CeremonyDesignDocument, CeremonyDesignStage};
use crate::usecases::CeremonyDesignExitGuard;

pub(super) fn stage_config(
    document: &CeremonyDesignDocument,
    entry_id: &StepId,
    stage: &CeremonyDesignStage,
    index: usize,
) -> BTreeMap<String, Value> {
    let mut config = BTreeMap::from([
        ("num_agents".to_owned(), json!(num_agents(stage))),
        ("prompt".to_owned(), json!(stage.instructions().trim())),
        (
            // Earlier stages are context by default for everything
            // after the first, which has nothing to see.
            "see_prior".to_owned(),
            json!(stage.prior_context().map_or(
                index > 0,
                made_core::value_objects::PriorContext::is_visible
            )),
        ),
    ]);
    if stage.review_rounds().get() > 0 {
        config.insert("rounds".to_owned(), json!(stage.review_rounds().get()));
    }
    let projected = projected_winner_fields(document, entry_id, stage);
    if !projected.is_empty() {
        config.insert("project_winner_fields".to_owned(), json!(projected));
    }
    config
}

fn projected_winner_fields(
    document: &CeremonyDesignDocument,
    entry_id: &StepId,
    stage: &CeremonyDesignStage,
) -> Vec<String> {
    if document.state_pattern(entry_id).is_none() {
        return Vec::new();
    }
    let from_context = stage
        .context_writes()
        .entries()
        .values()
        .map(|field| field.as_str().to_owned());
    let from_exit_guards = document.stages().iter().flat_map(|candidate| {
        candidate
            .exit_guards()
            .iter()
            .filter_map(|guard| match guard {
                CeremonyDesignExitGuard::OutputField(guard) if guard.step_id() == stage.id() => {
                    Some(guard.output_field().as_str().to_owned())
                }
                _ => None,
            })
    });
    let from_routes = document.routes().iter().flat_map(|route| {
        route
            .guards()
            .iter()
            .filter_map(|(_, condition)| match condition {
                GuardCondition::OutputField(guard) if guard.step_id() == stage.id() => {
                    Some(guard.output_field().as_str().to_owned())
                }
                _ => None,
            })
    });
    from_context
        .chain(from_exit_guards)
        .chain(from_routes)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
