use made_adapters::scoring::{JudgeAwareScoring, UniformScoring};
use made_core::error::DomainError;
use made_core::ports::{MetricsRecorderPort, ScoringPort, ValidatorPort};
use std::sync::Arc;
use tracing::info;
/// Pick the scoring policy and wire the optional LLM judge.
///
/// When `judge_from_env` yields a judge it is appended to `validators`,
/// and `JudgeAwareScoring` makes its verdict rank proposals; otherwise
/// scoring is uniform. Fails fast when the judge is enabled but
/// misconfigured.
pub(super) fn wire(
    validators: &mut Vec<Arc<dyn ValidatorPort>>,
    metrics: Arc<dyn MetricsRecorderPort>,
) -> Result<Arc<dyn ScoringPort>, DomainError> {
    match made_adapters::agents::judge_from_env(metrics.clone())? {
        Some(judge) => {
            validators.push(judge);
            info!("scoring: LLM judge enabled; ranking by judge verdict");
            Ok(Arc::new(JudgeAwareScoring::new().with_metrics(metrics)))
        }
        None => Ok(Arc::new(UniformScoring::new())),
    }
}
