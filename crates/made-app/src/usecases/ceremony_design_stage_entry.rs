use super::{CeremonyDesignGroup, CeremonyDesignStage};

#[derive(Debug, Clone, PartialEq)]
pub enum CeremonyDesignStageEntry {
    Leaf(CeremonyDesignStage),
    Group(CeremonyDesignGroup),
}
