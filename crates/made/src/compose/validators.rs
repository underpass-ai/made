//! Validator policy installed by the service host.
use crate::ComposeError;
use made_adapters::metrics::PrometheusMetricsRecorder;
use made_adapters::validators::{
    AllowedStringValuesValidator, BoundedEventShapeValidator, ClaimsEvidenceGroundedValidator,
    ClaimsEvidenceSupportedValidator, ContentNonEmptyValidator, JsonObjectOutputValidator,
    JsonSchemaValidator, RequiredFieldsValidator,
};
use made_core::ports::ValidatorPort;
use std::sync::Arc;
pub(super) fn wire(
    metrics_recorder: Arc<PrometheusMetricsRecorder>,
) -> Result<Vec<Arc<dyn ValidatorPort>>, ComposeError> {
    let validators: Vec<Arc<dyn ValidatorPort>> = vec![
        Arc::new(ContentNonEmptyValidator::new()),
        Arc::new(JsonObjectOutputValidator::new()),
        Arc::new(RequiredFieldsValidator::new()),
        Arc::new(AllowedStringValuesValidator::new()),
        Arc::new(JsonSchemaValidator::new()),
        // Evidence grounding: rejects claims citing refs outside the
        // contract's evidence pack (no-op unless a contract declares a
        // grounding rule). Runs before the shape-budget guard so orphan
        // refs are named even when the output is otherwise well-formed.
        Arc::new(ClaimsEvidenceGroundedValidator::new()),
        // Semantic support: rejects claims whose *cited* evidence does
        // not actually support them, judged through the deployment's
        // evidence-support judge (`MADE_SUPPORT_JUDGE_ENABLED`, vLLM
        // endpoint/model). No-op unless a contract declares
        // `evidence.semantic_support`; a contract that demands it with
        // no judge wired fails its step loudly instead of running the
        // gate voided.
        Arc::new(ClaimsEvidenceSupportedValidator::new(
            made_adapters::agents::support_judge_from_env(metrics_recorder)?,
        )),
        // Final shape-budget guard: defends downstream bus consumers
        // against pathological JSON (deeply nested, huge arrays,
        // bloated strings). Uses the validator's conservative
        // defaults; tune with the `with_*` builders when a deploy
        // needs different bounds.
        Arc::new(BoundedEventShapeValidator::new()),
    ];
    Ok(validators)
}
