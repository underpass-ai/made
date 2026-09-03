use async_trait::async_trait;
use made_core::entities::ValidatorReport;
use made_core::error::DomainError;
use made_core::ports::ScoringPort;
use made_core::value_objects::Score;

/// Uniform scoring: the score is the fraction of reports that passed.
///
/// With zero reports the score is [`Score::MIN`] — no evidence means
/// no confidence. That choice biases operators to configure at least
/// one validator rather than silently returning a perfect score from
/// thin air.
#[derive(Debug, Default, Clone, Copy)]
pub struct UniformScoring;

impl UniformScoring {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ScoringPort for UniformScoring {
    async fn score(&self, reports: &[ValidatorReport]) -> Result<Score, DomainError> {
        if reports.is_empty() {
            return Ok(Score::MIN);
        }
        let passed = reports.iter().filter(|report| report.passed()).count();
        let total = reports.len();
        #[allow(clippy::cast_precision_loss)]
        let value = passed as f64 / total as f64;
        Score::new(value)
    }
}
