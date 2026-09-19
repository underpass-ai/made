//! Provider operation identity, correlation and observation contracts.

#![allow(clippy::result_large_err, clippy::struct_field_names)]

use std::fmt;

use made_core::value_objects::{DurationMs, LlmErrorKind, TokenUsage};
use thiserror::Error;

/// The non-secret identity that must survive a provider operation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderIdentity {
    operation_id: ProviderOperationId,
    provider: ProviderName,
    model: ProviderModel,
}

impl ProviderIdentity {
    /// Build an identity from the operation, provider and model labels.
    pub fn new(
        operation_id: impl Into<String>,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self, ProviderContractError> {
        Ok(Self {
            operation_id: ProviderOperationId::new(operation_id)?,
            provider: ProviderName::new(provider)?,
            model: ProviderModel::new(model)?,
        })
    }

    /// Stable semantic operation identity.
    #[must_use]
    pub fn operation_id(&self) -> &ProviderOperationId {
        &self.operation_id
    }

    /// Provider profile identity.
    #[must_use]
    pub fn provider(&self) -> &ProviderName {
        &self.provider
    }

    /// Model identity within the provider profile.
    #[must_use]
    pub fn model(&self) -> &ProviderModel {
        &self.model
    }
}

/// Correlation that ties a provider request back to the caller and its trace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OperationCorrelation {
    request_id: ProviderRequestId,
    correlation_id: Option<ProviderRequestId>,
}

/// Compatibility name emphasizing that correlation belongs to the request.
pub type RequestCorrelation = OperationCorrelation;

impl OperationCorrelation {
    /// Build correlation with a required request id and optional parent id.
    pub fn new(request_id: impl Into<String>) -> Result<Self, ProviderContractError> {
        Ok(Self {
            request_id: ProviderRequestId::new(request_id)?,
            correlation_id: None,
        })
    }

    /// Attach the caller's parent correlation id.
    pub fn with_correlation_id(
        mut self,
        correlation_id: impl Into<String>,
    ) -> Result<Self, ProviderContractError> {
        self.correlation_id = Some(ProviderRequestId::new(correlation_id)?);
        Ok(self)
    }

    /// Request id assigned by the caller.
    #[must_use]
    pub fn request_id(&self) -> &ProviderRequestId {
        &self.request_id
    }

    /// Optional parent/request-chain correlation id.
    #[must_use]
    pub fn correlation_id(&self) -> Option<&ProviderRequestId> {
        self.correlation_id.as_ref()
    }
}

/// A request handed to a provider operation adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperationRequest {
    identity: ProviderIdentity,
    correlation: OperationCorrelation,
}

impl ProviderOperationRequest {
    /// Create a request whose identity and correlation must be preserved by the adapter.
    #[must_use]
    pub const fn new(identity: ProviderIdentity, correlation: OperationCorrelation) -> Self {
        Self {
            identity,
            correlation,
        }
    }

    /// Operation/provider/model identity.
    #[must_use]
    pub fn identity(&self) -> &ProviderIdentity {
        &self.identity
    }

    /// Caller/request correlation.
    #[must_use]
    pub fn correlation(&self) -> &OperationCorrelation {
        &self.correlation
    }
}

/// Result of observing one provider operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperationObservation {
    request: ProviderOperationRequest,
    latency: Observed<DurationMs>,
    usage: Observed<Usage>,
    finish: Observed<FinishClassification>,
}

/// Latency with explicit declared/external provenance.
pub type ObservedLatency = Observed<DurationMs>;

/// Usage with explicit declared/external provenance.
pub type ObservedUsage = Observed<Usage>;

/// Finish/error classification with explicit declared/external provenance.
pub type ObservedFinish = Observed<FinishClassification>;

impl ProviderOperationObservation {
    /// Construct an observation while retaining the complete request envelope.
    #[must_use]
    pub const fn new(
        request: ProviderOperationRequest,
        latency: Observed<DurationMs>,
        usage: Observed<Usage>,
        finish: Observed<FinishClassification>,
    ) -> Self {
        Self {
            request,
            latency,
            usage,
            finish,
        }
    }

    /// Original request identity, including operation, provider and model.
    #[must_use]
    pub fn identity(&self) -> &ProviderIdentity {
        self.request.identity()
    }

    /// Original request correlation.
    #[must_use]
    pub fn correlation(&self) -> &OperationCorrelation {
        self.request.correlation()
    }

    /// Latency observation and its origin.
    #[must_use]
    pub fn latency(&self) -> &Observed<DurationMs> {
        &self.latency
    }

    /// Token/usage observation and its origin.
    #[must_use]
    pub fn usage(&self) -> &Observed<Usage> {
        &self.usage
    }

    /// Finish or error classification and its origin.
    #[must_use]
    pub fn finish(&self) -> &Observed<FinishClassification> {
        &self.finish
    }
}

/// An adapter that observes one provider operation without owning authority over it.
pub trait ProviderOperationAdapter: fmt::Debug + Send + Sync {
    /// Execute/observe a provider request.
    fn execute(
        &self,
        request: &ProviderOperationRequest,
    ) -> Result<ProviderOperationObservation, ProviderContractError>;
}

/// A value paired with the origin that gave it meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observed<T> {
    value: T,
    origin: ObservationOrigin,
}

impl<T> Observed<T> {
    /// Mark an observation as declared by a provider.
    #[must_use]
    pub const fn declared_by_provider(value: T) -> Self {
        Self {
            value,
            origin: ObservationOrigin::Declared {
                by: ObservationDeclarer::Provider,
            },
        }
    }

    /// Mark an observation as declared by an adapter.
    #[must_use]
    pub const fn declared_by_adapter(value: T) -> Self {
        Self {
            value,
            origin: ObservationOrigin::Declared {
                by: ObservationDeclarer::Adapter,
            },
        }
    }

    /// Mark an observation as declared by a deterministic fixture.
    #[must_use]
    pub const fn declared_by_fixture(value: T) -> Self {
        Self {
            value,
            origin: ObservationOrigin::Declared {
                by: ObservationDeclarer::Fixture,
            },
        }
    }

    /// Mark a value as backed by an explicitly named external authority.
    pub fn from_external_authority(
        value: T,
        authority: impl Into<String>,
    ) -> Result<Self, ProviderContractError> {
        Ok(Self {
            value,
            origin: ObservationOrigin::ExternalAuthority {
                authority: ExternalAuthorityId::new(authority)?,
            },
        })
    }

    /// The observed value.
    #[must_use]
    pub const fn value(&self) -> &T {
        &self.value
    }

    /// The declared/external origin.
    #[must_use]
    pub const fn origin(&self) -> &ObservationOrigin {
        &self.origin
    }

    /// Whether the value is backed by an explicitly named external authority.
    #[must_use]
    pub const fn is_externally_authoritative(&self) -> bool {
        matches!(self.origin, ObservationOrigin::ExternalAuthority { .. })
    }

    /// Split the value and provenance without losing either part.
    #[must_use]
    pub fn into_parts(self) -> (T, ObservationOrigin) {
        (self.value, self.origin)
    }
}

/// Who declared an observation when it has no external authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationDeclarer {
    /// The provider response declared the value.
    Provider,
    /// The local adapter measured or classified the value.
    Adapter,
    /// A deterministic fixture supplied the value for a test.
    Fixture,
}

/// Provenance of a latency, usage or finish/error observation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObservationOrigin {
    /// A declaration/measurement that is useful evidence but not external authority.
    Declared { by: ObservationDeclarer },
    /// A named external system is authoritative for this value.
    ExternalAuthority { authority: ExternalAuthorityId },
}

/// Usage counts; `None` means the source did not report that dimension, not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

impl Usage {
    /// Build usage while preserving whether total was actually reported.
    #[must_use]
    pub const fn new(input_tokens: Option<u64>, output_tokens: Option<u64>) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }

    /// Add an explicitly reported total token count.
    #[must_use]
    pub const fn with_total(mut self, total_tokens: u64) -> Self {
        self.total_tokens = Some(total_tokens);
        self
    }

    /// Add an explicitly reported cache-read count.
    #[must_use]
    pub const fn with_cache_read(mut self, tokens: u64) -> Self {
        self.cache_read_tokens = Some(tokens);
        self
    }

    /// Add an explicitly reported cache-write count.
    #[must_use]
    pub const fn with_cache_write(mut self, tokens: u64) -> Self {
        self.cache_write_tokens = Some(tokens);
        self
    }

    #[must_use]
    pub const fn input_tokens(self) -> Option<u64> {
        self.input_tokens
    }

    #[must_use]
    pub const fn output_tokens(self) -> Option<u64> {
        self.output_tokens
    }

    #[must_use]
    pub const fn total_tokens(self) -> Option<u64> {
        self.total_tokens
    }

    #[must_use]
    pub const fn cache_read_tokens(self) -> Option<u64> {
        self.cache_read_tokens
    }

    #[must_use]
    pub const fn cache_write_tokens(self) -> Option<u64> {
        self.cache_write_tokens
    }
}

impl From<TokenUsage> for Usage {
    fn from(value: TokenUsage) -> Self {
        Self::new(
            Some(u64::from(value.prompt())),
            Some(u64::from(value.completion())),
        )
    }
}

/// Finish reason or low-cardinality provider failure classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishClassification {
    /// The provider completed the operation and reported why generation stopped.
    Finished(FinishReason),
    /// The operation failed and was classified without retaining unsafe raw detail.
    Error(ProviderErrorClassification),
}

/// Stable finish reasons suitable for evidence and metrics dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCall,
    ContentFilter,
    NotReported,
}

/// Low-cardinality error classification; raw provider bodies are intentionally absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderErrorClassification {
    Unauthorized,
    RateLimited,
    BadRequest,
    Upstream,
    MalformedResponse,
    EmptyResponse,
    Timeout,
    Transport,
    Cancelled,
    Unknown,
}

impl From<LlmErrorKind> for ProviderErrorClassification {
    fn from(value: LlmErrorKind) -> Self {
        match value {
            LlmErrorKind::Unauthorized => Self::Unauthorized,
            LlmErrorKind::RateLimited => Self::RateLimited,
            LlmErrorKind::BadRequest => Self::BadRequest,
            LlmErrorKind::UpstreamError => Self::Upstream,
            LlmErrorKind::MalformedBody => Self::MalformedResponse,
            LlmErrorKind::EmptyContent => Self::EmptyResponse,
            LlmErrorKind::Timeout => Self::Timeout,
            LlmErrorKind::Transport => Self::Transport,
        }
    }
}

/// A non-secret provider label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderName(String);

/// A non-secret model label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderModel(String);

/// Stable semantic provider operation id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderOperationId(String);

/// Caller-assigned request/correlation id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProviderRequestId(String);

/// Identifier of a system that is authoritative for an observation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalAuthorityId(String);

macro_rules! impl_label {
    ($name:ident, $field:literal) => {
        impl $name {
            pub fn new(raw: impl Into<String>) -> Result<Self, ProviderContractError> {
                let value = raw.into().trim().to_owned();
                if value.is_empty() {
                    return Err(ProviderContractError::EmptyField { field: $field });
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

impl_label!(ProviderName, "provider");
impl_label!(ProviderModel, "model");
impl_label!(ProviderOperationId, "operation_id");
impl_label!(ProviderRequestId, "request_id");
impl_label!(ExternalAuthorityId, "external_authority");

/// Opaque credential that can be carried by an adapter without leaking in debug output.
#[derive(Clone)]
pub struct ProviderSecret(String);

impl ProviderSecret {
    /// Reject empty credentials at the adapter boundary.
    pub fn new(raw: impl Into<String>) -> Result<Self, ProviderContractError> {
        let value = raw.into().trim().to_owned();
        if value.is_empty() {
            return Err(ProviderContractError::EmptyField { field: "secret" });
        }
        Ok(Self(value))
    }
}

impl fmt::Debug for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _secret_is_present = !self.0.is_empty();
        f.write_str("ProviderSecret(**redacted**)")
    }
}

impl fmt::Display for ProviderSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("**redacted**")
    }
}

/// Contract errors are safe to expose in logs and test snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProviderContractError {
    #[error("provider contract field `{field}` must not be empty")]
    EmptyField { field: &'static str },
    #[error("provider adapter identity mismatch: expected {expected:?}, got {actual:?}")]
    IdentityMismatch {
        expected: ProviderIdentity,
        actual: ProviderIdentity,
    },
}
