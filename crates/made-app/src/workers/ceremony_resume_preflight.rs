use super::{CeremonyClaimPreflight, CeremonyPreflightAction};
use made_core::value_objects::{CeremonyId, CeremonyLifecyclePhase, StepClaimFence, StreamVersion};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "This application-boundary projection deliberately exposes independent, source-labelled facts rather than collapsing them into a lifecycle state machine."
)]
pub struct CeremonyResumePreflight {
    pub ceremony_id: CeremonyId,
    pub journal_version: StreamVersion,
    #[serde(with = "time::serde::rfc3339")]
    pub inspected_at: OffsetDateTime,
    pub lifecycle: CeremonyLifecyclePhase,
    pub admission_paused: bool,
    /// Sealed engine fact; it is independent from host declarations.
    pub engine_drained: bool,
    /// Latest host fact for every still in-flight exact claim, never historical claims.
    pub all_claims_host_reported_quiesced: bool,
    /// Coordination readiness only; never an authorization or takeover permit.
    pub coordinated_resume_ready: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ceremony_deadline_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub state_deadline_at: Option<OffsetDateTime>,
    pub deadline_overdue: bool,
    pub claims: Vec<CeremonyClaimPreflight>,
    pub next_after_claim: Option<StepClaimFence>,
    pub permitted_recovery_paths: Vec<CeremonyPreflightAction>,
}
