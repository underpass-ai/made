//! One evidence source, both engines.
//!
//! `made_collect_ceremony_evidence` is a shared tool, and neither the
//! server's composition nor the in-process default ships a source for
//! it: both answer "not found" until an operator wires one. A tool that
//! only ever fails is a tool whose success is not compared, so the
//! parity session wires this one on both arms and collects a pack the
//! two answers must carry identically.
//!
//! The pack is a function of the request, so nothing about it depends
//! on which engine asked.

use std::sync::Arc;

use async_trait::async_trait;
use made_core::entities::{
    CeremonyEvidencePack, ContextItem, ContextSummary, ExternalContextBundle,
};
use made_core::error::DomainError;
use made_core::ports::{CeremonyEvidenceRequest, CeremonyEvidenceSourcePort};
use made_core::value_objects::Attributes;
use time::OffsetDateTime;

use crate::parity_clock::PARITY_INSTANT;

/// A [`CeremonyEvidenceSourcePort`] that answers from the request.
#[derive(Debug, Clone, Copy)]
pub struct ParityEvidenceSource {
    collected_at: OffsetDateTime,
}

impl Default for ParityEvidenceSource {
    fn default() -> Self {
        Self::at(PARITY_INSTANT)
    }
}

impl ParityEvidenceSource {
    #[must_use]
    pub const fn at(collected_at: OffsetDateTime) -> Self {
        Self { collected_at }
    }

    /// The source, ready to hand to a builder or a fixture.
    #[must_use]
    pub fn shared() -> Arc<dyn CeremonyEvidenceSourcePort> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl CeremonyEvidenceSourcePort for ParityEvidenceSource {
    async fn collect(
        &self,
        request: CeremonyEvidenceRequest,
    ) -> Result<CeremonyEvidencePack, DomainError> {
        let item = ContextItem::new(
            "parity-observation",
            "metric",
            "What the source found",
            Some(format!(
                "Asked `{}`; the answer is 18%.",
                request.query().message()
            )),
            Attributes::empty(),
            Vec::new(),
        )?;
        let bundle = ExternalContextBundle::new(
            request.source_id().as_str(),
            "1.0",
            Some(ContextSummary::new(
                "The source answered the investigation.",
                Attributes::empty(),
            )?),
            vec![item],
            Vec::new(),
            Attributes::empty(),
        )?;
        CeremonyEvidencePack::new(request.source_id().clone(), bundle, self.collected_at)
    }
}
