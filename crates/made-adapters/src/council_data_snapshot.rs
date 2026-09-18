//! Offline import/export DTO. This is a data snapshot, not journal replay or a
//! backup of cursors. Whole-store backup remains the operator's restore path.
use made_core::entities::{Council, CouncilSnapshotProvenance, Deliberation, Statistics};
use made_core::error::DomainError;
use made_core::ports::AgentDescriptor;
use made_core::value_objects::OutputContract;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CouncilDataSnapshot {
    pub schema_version: u32,
    pub provenance: CouncilSnapshotProvenance,
    pub councils: Vec<Council>,
    pub agents: Vec<AgentDescriptor>,
    pub contracts: Vec<OutputContract>,
    pub deliberations: Vec<Deliberation>,
    pub statistics: Statistics,
}
impl CouncilDataSnapshot {
    pub const MAX_BYTES: usize = 32 * 1024 * 1024;
    pub fn decode(bytes: &[u8]) -> Result<Self, DomainError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(invalid());
        }
        let snapshot: Self = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        snapshot.validate()?;
        Ok(snapshot)
    }
    pub fn encode(&self) -> Result<Vec<u8>, DomainError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| invalid())?;
        if bytes.len() > Self::MAX_BYTES {
            return Err(invalid());
        }
        Ok(bytes)
    }
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.schema_version != 1 {
            return Err(invalid());
        }
        unique(self.councils.iter().map(|v| v.specialty().as_str()))?;
        unique(self.agents.iter().map(|v| v.id.as_str()))?;
        unique(self.contracts.iter().map(|v| v.contract_id().as_str()))?;
        unique(self.deliberations.iter().map(|v| v.task_id().as_str()))?;
        crate::council_snapshot_validation::validate(self)
    }
    pub(crate) fn canonicalized(mut self) -> Self {
        self.councils
            .sort_by(|a, b| a.specialty().cmp(b.specialty()));
        self.agents.sort_by(|a, b| a.id.cmp(&b.id));
        self.contracts
            .sort_by(|a, b| a.contract_id().cmp(b.contract_id()));
        self.deliberations
            .sort_by(|a, b| a.task_id().cmp(b.task_id()));
        self
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.councils.is_empty()
            && self.agents.is_empty()
            && self.contracts.is_empty()
            && self.deliberations.is_empty()
            && self.statistics == Statistics::default()
    }
}
fn unique<'a>(keys: impl Iterator<Item = &'a str>) -> Result<(), DomainError> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if seen.len() >= 100_000 || !seen.insert(key) {
            return Err(invalid());
        }
    }
    Ok(())
}
fn invalid() -> DomainError {
    DomainError::InvariantViolated {
        reason: "invalid council snapshot schema, size or duplicate identity",
    }
}
