use super::*;
use made_core::value_objects::{DurationMs, LlmErrorKind, TokenUsage};

fn request() -> ProviderOperationRequest {
    let identity = ProviderIdentity::new("op-42", "acme", "model-v1").unwrap();
    let correlation = OperationCorrelation::new("req-7")
        .unwrap()
        .with_correlation_id("trace-9")
        .unwrap();
    ProviderOperationRequest::new(identity, correlation)
}

#[test]
fn fake_preserves_identity_and_request_correlation() {
    let adapter = FakeProviderAdapter::new(
        "acme",
        "model-v1",
        FakeProviderResponse::success(
            125,
            Usage::new(Some(100), Some(25)).with_total(125),
            FinishReason::Stop,
        ),
    )
    .unwrap();

    let observation = adapter.execute(&request()).unwrap();
    assert_eq!(observation.identity().operation_id().as_str(), "op-42");
    assert_eq!(observation.identity().provider().as_str(), "acme");
    assert_eq!(observation.identity().model().as_str(), "model-v1");
    assert_eq!(observation.correlation().request_id().as_str(), "req-7");
    assert_eq!(
        observation.correlation().correlation_id().unwrap().as_str(),
        "trace-9"
    );
    assert_eq!(observation.latency().value(), &DurationMs::from_millis(125));
    assert_eq!(observation.usage().value().total_tokens(), Some(125));
    assert_eq!(
        observation.finish().value(),
        &FinishClassification::Finished(FinishReason::Stop)
    );
}

#[test]
fn missing_usage_is_not_converted_to_zero_and_fixture_is_not_authority() {
    let adapter = FakeProviderAdapter::new(
        "acme",
        "model-v1",
        FakeProviderResponse::success(4, Usage::default(), FinishReason::NotReported),
    )
    .unwrap();

    let observation = adapter.execute(&request()).unwrap();
    assert_eq!(observation.usage().value().input_tokens(), None);
    assert_eq!(observation.usage().value().output_tokens(), None);
    assert!(!observation.usage().is_externally_authoritative());
    assert_eq!(
        observation.usage().origin(),
        &ObservationOrigin::Declared {
            by: ObservationDeclarer::Fixture
        }
    );
}

#[test]
fn provider_error_is_classified_without_raw_error_text() {
    let adapter = FakeProviderAdapter::new(
        "acme",
        "model-v1",
        FakeProviderResponse::error(800, Usage::default(), ProviderErrorClassification::Timeout),
    )
    .unwrap();

    let observation = adapter.execute(&request()).unwrap();
    assert_eq!(
        observation.finish().value(),
        &FinishClassification::Error(ProviderErrorClassification::Timeout)
    );
}

#[test]
fn identity_mismatch_is_rejected_before_an_observation_is_created() {
    let adapter = FakeProviderAdapter::new(
        "other-provider",
        "model-v1",
        FakeProviderResponse::success(1, Usage::default(), FinishReason::Stop),
    )
    .unwrap();

    let error = adapter.execute(&request()).unwrap_err();
    assert!(matches!(
        error,
        ProviderContractError::IdentityMismatch { .. }
    ));
}

#[test]
fn external_authority_is_explicit_and_named() {
    let observed = Observed::from_external_authority(Usage::default(), "billing-ledger").unwrap();
    assert!(observed.is_externally_authoritative());
    assert_eq!(
        observed.origin(),
        &ObservationOrigin::ExternalAuthority {
            authority: ExternalAuthorityId::new("billing-ledger").unwrap()
        }
    );
}

#[test]
fn existing_core_usage_and_error_types_map_without_inventing_totals() {
    let usage = Usage::from(TokenUsage::new(9, 4));
    assert_eq!(usage.input_tokens(), Some(9));
    assert_eq!(usage.output_tokens(), Some(4));
    assert_eq!(usage.total_tokens(), None);
    assert_eq!(
        ProviderErrorClassification::from(LlmErrorKind::MalformedBody),
        ProviderErrorClassification::MalformedResponse
    );
}

#[test]
fn secret_debug_is_redacted_even_when_held_by_the_fake() {
    let secret = "provider-secret-123";
    let adapter = FakeProviderAdapter::new(
        "acme",
        "model-v1",
        FakeProviderResponse::success(1, Usage::default(), FinishReason::Stop),
    )
    .unwrap()
    .with_secret(ProviderSecret::new(secret).unwrap());

    let debug = format!("{adapter:?}");
    assert!(!debug.contains(secret));
    assert!(debug.contains("redacted"));

    let credential = ProviderSecret::new(secret).unwrap();
    assert!(!credential.to_string().contains(secret));
    assert!(credential.to_string().contains("redacted"));
}
