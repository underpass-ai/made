use super::{ProtocolDesignGroup, ProtocolDesignPatternStage, ProtocolDesignStage};

#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolDesignStageEntry {
    Leaf(ProtocolDesignStage),
    Group(ProtocolDesignGroup),
    Pattern(ProtocolDesignPatternStage),
}
