use super::CeremonyDesignStage;

#[derive(Debug, Clone, PartialEq)]
pub struct CeremonyDesignGroupStep(CeremonyDesignStage);

impl CeremonyDesignGroupStep {
    #[must_use]
    pub fn new(step: CeremonyDesignStage) -> Self {
        Self(step)
    }
    #[must_use]
    pub fn step(&self) -> &CeremonyDesignStage {
        &self.0
    }
}
