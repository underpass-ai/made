use made_core::value_objects::JoinStepCount;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProtocolDesignJoin {
    #[default]
    AllStepsCompleted,
    AnyStepCompleted,
    StepsCompleted(JoinStepCount),
}
