use made_core::value_objects::JoinStepCount;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CeremonyDesignJoin {
    #[default]
    AllStepsCompleted,
    AnyStepCompleted,
    StepsCompleted(JoinStepCount),
}
