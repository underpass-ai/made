//! Provider operation identity, correlation and observation contracts.

#![allow(clippy::result_large_err, clippy::struct_field_names)]

mod external_authority_id;
mod finish_classification;
mod finish_reason;
mod observation_declarer;
mod observation_origin;
mod observed;
mod observed_finish;
mod observed_latency;
mod observed_usage;
mod operation_correlation;
mod provider_contract_error;
mod provider_error_classification;
mod provider_identity;
mod provider_model;
mod provider_name;
mod provider_operation_adapter;
mod provider_operation_id;
mod provider_operation_observation;
mod provider_operation_request;
mod provider_request_id;
mod provider_secret;
mod request_correlation;
mod usage;

pub use external_authority_id::ExternalAuthorityId;
pub use finish_classification::FinishClassification;
pub use finish_reason::FinishReason;
pub use observation_declarer::ObservationDeclarer;
pub use observation_origin::ObservationOrigin;
pub use observed::Observed;
pub use observed_finish::ObservedFinish;
pub use observed_latency::ObservedLatency;
pub use observed_usage::ObservedUsage;
pub use operation_correlation::OperationCorrelation;
pub use provider_contract_error::ProviderContractError;
pub use provider_error_classification::ProviderErrorClassification;
pub use provider_identity::ProviderIdentity;
pub use provider_model::ProviderModel;
pub use provider_name::ProviderName;
pub use provider_operation_adapter::ProviderOperationAdapter;
pub use provider_operation_id::ProviderOperationId;
pub use provider_operation_observation::ProviderOperationObservation;
pub use provider_operation_request::ProviderOperationRequest;
pub use provider_request_id::ProviderRequestId;
pub use provider_secret::ProviderSecret;
pub use request_correlation::RequestCorrelation;
pub use usage::Usage;

macro_rules! define_label {
    ($name:ident, $field:literal) => {
        impl $name {
            pub fn new(raw: impl Into<String>) -> Result<Self, super::ProviderContractError> {
                let value = raw.into().trim().to_owned();
                if value.is_empty() {
                    return Err(super::ProviderContractError::EmptyField { field: $field });
                }
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
pub(super) use define_label;
