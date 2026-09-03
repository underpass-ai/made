//! [`EvidenceSupportJudgePort`] — does a claim's cited evidence
//! actually support the claim?
//!
//! The grounding gate (`claims-evidence-grounded`) proves a citation
//! *exists*; this port answers whether the citation *holds*. That is a
//! semantic judgment, so implementations are typically model-backed
//! (an LLM or NLI adapter) — but the port keeps the core
//! provider-agnostic, exactly like [`super::AgentPort`], and the
//! *decision* stays deterministic: the validator compares the verdict
//! against the contract's configured threshold and records the verdict
//! itself in the report, so the model's opinion becomes evidence in
//! the decision record rather than the last word.

use async_trait::async_trait;

use crate::error::DomainError;
use crate::value_objects::{ClaimText, EvidenceExcerpt, SupportVerdict};

#[async_trait]
pub trait EvidenceSupportJudgePort: Send + Sync {
    /// Assess whether `evidence` supports `claim_text`. `evidence`
    /// carries only the excerpts the claim actually cited — the judge
    /// must not be able to lean on evidence the claim did not invoke.
    async fn assess(
        &self,
        claim: &ClaimText,
        evidence: &[EvidenceExcerpt],
    ) -> Result<SupportVerdict, DomainError>;
}
