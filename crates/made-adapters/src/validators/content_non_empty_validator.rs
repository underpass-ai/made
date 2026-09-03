use async_trait::async_trait;
use made_core::entities::{TaskConstraints, ValidatorReport};
use made_core::error::DomainError;
use made_core::ports::ValidatorPort;
use made_core::value_objects::Attributes;

#[derive(Debug, Default, Clone, Copy)]
pub struct ContentNonEmptyValidator;

/// A validator that fails a proposal only when its content is empty
/// (after trimming). The thinnest possible "is there anything here"
/// check — useful as a default so operators who do not configure a
/// richer validator still get a meaningful ranking signal.
impl ContentNonEmptyValidator {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorPort for ContentNonEmptyValidator {
    fn kind(&self) -> &'static str {
        "content-non-empty"
    }

    async fn validate(
        &self,
        proposal_content: &str,
        _constraints: &TaskConstraints,
    ) -> Result<ValidatorReport, DomainError> {
        let trimmed = proposal_content.trim();
        let passed = !trimmed.is_empty();
        let summary = if passed {
            format!("len={}", trimmed.len())
        } else {
            "content is empty after trimming".to_owned()
        };
        ValidatorReport::new(self.kind(), passed, summary, Attributes::empty())
    }
}
