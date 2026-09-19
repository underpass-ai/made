//! Typed provider-operation contracts and deterministic test adapters.
//!
//! This module deliberately stops at the observation boundary. A provider or
//! adapter may declare what it saw, but a declaration is not an externally
//! authoritative bill, quota decision, or reconciliation result. The
//! [`ObservationOrigin`] carried by every observation keeps that distinction
//! explicit for callers and tests.

#![allow(clippy::result_large_err, clippy::struct_field_names)]

mod fake;
mod operation;

pub use fake::{DeterministicProviderAdapter, FakeProviderAdapter, FakeProviderResponse};
pub use operation::{
    ExternalAuthorityId, FinishClassification, FinishReason, ObservationDeclarer,
    ObservationOrigin, Observed, ObservedFinish, ObservedLatency, ObservedUsage,
    OperationCorrelation, ProviderContractError, ProviderErrorClassification, ProviderIdentity,
    ProviderModel, ProviderName, ProviderOperationAdapter, ProviderOperationId,
    ProviderOperationObservation, ProviderOperationRequest, ProviderRequestId, ProviderSecret,
    RequestCorrelation, Usage,
};

#[cfg(test)]
mod tests;
