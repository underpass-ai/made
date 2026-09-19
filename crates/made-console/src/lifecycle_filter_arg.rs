use clap::ValueEnum;
use made_client::v1::CeremonyLifecycleFilter;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LifecycleFilterArg {
    Running,
    Paused,
    Ended,
}

impl From<LifecycleFilterArg> for CeremonyLifecycleFilter {
    fn from(value: LifecycleFilterArg) -> Self {
        match value {
            LifecycleFilterArg::Running => Self::Running,
            LifecycleFilterArg::Paused => Self::Paused,
            LifecycleFilterArg::Ended => Self::Ended,
        }
    }
}
