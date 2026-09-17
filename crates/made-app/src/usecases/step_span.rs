use made_core::error::DomainError;
use made_core::value_objects::{StepAttempt, StepIteration, StepResult};

pub(super) fn record_coordinates(iteration: StepIteration, attempt: StepAttempt) {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let context = tracing::Span::current().context();
    let otel_span = context.span();
    otel_span.set_attribute(opentelemetry::KeyValue::new(
        "iteration",
        i64::from(iteration.get()),
    ));
    otel_span.set_attribute(opentelemetry::KeyValue::new(
        "attempt",
        i64::from(attempt.get()),
    ));
}

pub(super) fn record_attempt(attempt: StepAttempt) {
    use opentelemetry::trace::TraceContextExt as _;
    use tracing_opentelemetry::OpenTelemetrySpanExt as _;

    let context = tracing::Span::current().context();
    context.span().set_attribute(opentelemetry::KeyValue::new(
        "attempt",
        i64::from(attempt.get()),
    ));
}

pub(super) fn record_result(result: &StepResult) {
    let span = tracing::Span::current();
    let status = result.status();
    span.record("step_status", status.as_label());
    if status.is_success() {
        span.record("outcome", "success");
    } else {
        span.record("outcome", "error");
        span.record("error_kind", "step_result_failed");
    }
}

pub(super) fn record_status(result: &StepResult) {
    tracing::Span::current().record("step_status", result.status().as_label());
}

pub(super) fn record_error(error: &DomainError) {
    let span = tracing::Span::current();
    span.record("outcome", "error");
    span.record("error_kind", domain_error_kind(error));
}

const fn domain_error_kind(error: &DomainError) -> &'static str {
    match error {
        DomainError::EmptyField { .. } => "empty_field",
        DomainError::FieldTooLong { .. } => "field_too_long",
        DomainError::InvalidCharacters { .. } => "invalid_characters",
        DomainError::OutOfRange { .. } => "out_of_range",
        DomainError::MustBeNonZero { .. } => "must_be_non_zero",
        DomainError::EmptyCollection { .. } => "empty_collection",
        DomainError::InvalidTransition { .. } => "invalid_transition",
        DomainError::InvariantViolated { .. } => "invariant_violated",
        DomainError::NotFound { .. } => "not_found",
        DomainError::InvalidDocument { .. } => "invalid_document",
        DomainError::AlreadyExists { .. } => "already_exists",
        DomainError::Conflict { .. } => "conflict",
        DomainError::NoValidProposal { .. } => "no_valid_proposal",
        DomainError::UnreadableCeremonyEvent { .. } => "unreadable_ceremony_event",
    }
}
