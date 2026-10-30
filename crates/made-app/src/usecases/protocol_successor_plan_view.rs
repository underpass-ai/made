use made_core::value_objects::{DefinitionPin, ProtocolDefinitionChange, ProtocolDefinitionDiff};

use crate::workers::ProtocolResumePreflight;

use super::{ProposedCarriedStep, ProtocolSuccessionBlocker, RequiredClaimDisposition};

/// The answer to "could this protocol hand off, and to what".
///
/// Read-only and complete on purpose: the diff that motivates the
/// handoff, the preflight that says what is still outstanding, the
/// evidence a successor could start from, the dispositions it would
/// require, and what is in the way. Deciding to hand off with this in
/// hand is a decision; deciding without it is a guess, and the whole
/// reason planning is a separate capability is that starting a
/// successor should not be how a caller finds out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSuccessorPlanView {
    pub predecessor_definition: DefinitionPin,
    pub successor_definition: DefinitionPin,
    pub diff: ProtocolDefinitionDiff,
    pub preflight: ProtocolResumePreflight,
    pub proposed_carried: Vec<ProposedCarriedStep>,
    pub required_dispositions: Vec<RequiredClaimDisposition>,
    /// The changes that would strand work this protocol has already
    /// completed. Not every strand in the diff: a strand over a step
    /// nobody ran costs nothing.
    pub strands: Vec<ProtocolDefinitionChange>,
    pub blockers: Vec<ProtocolSuccessionBlocker>,
    pub ready: bool,
}
