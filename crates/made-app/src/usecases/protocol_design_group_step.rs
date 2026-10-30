use super::ProtocolDesignStage;

#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolDesignGroupStep(ProtocolDesignStage);

impl ProtocolDesignGroupStep {
    #[must_use]
    pub fn new(step: ProtocolDesignStage) -> Self {
        Self(step)
    }
    #[must_use]
    pub fn step(&self) -> &ProtocolDesignStage {
        &self.0
    }
}
