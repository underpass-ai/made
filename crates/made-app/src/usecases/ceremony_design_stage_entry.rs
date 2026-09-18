use super::{CeremonyDesignGroup, CeremonyDesignPatternStage, CeremonyDesignStage};

#[derive(Debug, Clone, PartialEq)]
pub enum CeremonyDesignStageEntry {
    Leaf(CeremonyDesignStage),
    Group(CeremonyDesignGroup),
    Pattern(CeremonyDesignPatternStage),
}
