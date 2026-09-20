use time::OffsetDateTime;

use crate::entities::PublishedCeremonyDefinition;
use crate::value_objects::{CeremonyDefinitionDiff, SuccessionPlan};

/// Seal a handoff in the ceremony that is handing off.
///
/// The successor's published definition and the diff between the two
/// travel with the plan rather than being fetched by the decision: the
/// aggregate holds neither the catalogue nor the other definition, and
/// reading either would be I/O inside a pure decision. What stays in
/// the domain is what is done with them — the pin has to name the
/// bytes the plan was made against, evidence may only be carried onto
/// a step the successor actually declares, and only where the diff
/// says that step carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSuccessor {
    pub plan: SuccessionPlan,
    pub successor: Box<PublishedCeremonyDefinition>,
    pub diff: CeremonyDefinitionDiff,
    pub now: OffsetDateTime,
}
