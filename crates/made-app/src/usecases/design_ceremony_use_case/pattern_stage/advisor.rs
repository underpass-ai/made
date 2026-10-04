//! The advisor pattern: an executor consults a reviewer who sees the whole
//! transcript before substantive work and again when it believes the work is
//! done. The reviewer never does the work; it says whether a concern blocks.

use made_core::error::DomainError;
use made_core::value_objects::StepId;

use super::super::pattern_stage_routes::output_route;
use super::{fallback, group, id, invalid, leaf, Expansion};
use crate::usecases::{CeremonyDesignPatternStage, CeremonyDesignStageEntry};

const ORIENT: &str = "Orient before substantive work: read the material, locate what the \
                      task touches and state the approach you intend. Do not write the \
                      deliverable yet.";
const ADVISE: &str = "Advise before substantive work begins. Review the orientation and the \
                      whole transcript; name the approach to take and the traps to avoid.";
const WORK: &str = "Make the deliverable durable before returning. Give the advice serious \
                    weight; where your evidence contradicts it, say so and show the evidence.";
const REVIEW: &str = "Review the durable deliverable against the whole transcript. Return \
                      blocks (true only when a concern changes the result) plus advice.";

pub(super) fn advisor(pattern: &CeremonyDesignPatternStage) -> Result<Expansion, DomainError> {
    let [executor, reviewer] = pattern.roles() else {
        return Err(invalid(format!(
            "advisor pattern `{}` requires exactly executor and advisor roles",
            pattern.id()
        )));
    };
    let fallback = fallback(pattern)?;
    let cap = pattern.max_iterations().expect("validated cap").get();
    let deliver_id = id(pattern, "deliver")?;
    let fallback_id = id(pattern, "fallback")?;
    let work = format!("{} {WORK}", pattern.instructions().trim());
    let mut entries = vec![group(
        id(pattern, "plan")?,
        vec![
            leaf(id(pattern, "orient")?.as_str(), executor, ORIENT, true)?,
            leaf(id(pattern, "advise")?.as_str(), reviewer, ADVISE, true)?,
        ],
    )];
    let mut routes = Vec::new();
    for iteration in 1..=cap {
        let state_id = StepId::new(format!("{}_iteration_{iteration}", pattern.id()))?;
        let work_id = StepId::new(format!("{}_work_{iteration}", pattern.id()))?;
        let review_id = StepId::new(format!("{}_review_{iteration}", pattern.id()))?;
        entries.push(group(
            state_id.clone(),
            vec![
                leaf(work_id.as_str(), executor, &work, true)?,
                leaf(review_id.as_str(), reviewer, REVIEW, true)?,
            ],
        ));
        routes.push(output_route(
            &state_id,
            &deliver_id,
            reviewer,
            &review_id,
            &review_id,
            "blocks",
            false,
            "clear",
        )?);
        let next = if iteration == cap {
            fallback_id.clone()
        } else {
            StepId::new(format!("{}_iteration_{}", pattern.id(), iteration + 1))?
        };
        routes.push(output_route(
            &state_id, &next, reviewer, &review_id, &review_id, "blocks", true, "blocks",
        )?);
    }
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        fallback_id.as_str(),
        fallback,
        "Decide on work whose advisor concerns still block at the declared cap.",
        true,
    )?));
    entries.push(CeremonyDesignStageEntry::Leaf(leaf(
        deliver_id.as_str(),
        executor,
        "Deliver the reviewed result, retaining any advice left unresolved.",
        true,
    )?));
    Ok(Expansion { entries, routes })
}
