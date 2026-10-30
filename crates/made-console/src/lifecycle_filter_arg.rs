use clap::ValueEnum;
use made_client::v1::ProtocolLifecycleFilter;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LifecycleFilterArg {
    Running,
    Paused,
    Ended,
}

impl From<LifecycleFilterArg> for ProtocolLifecycleFilter {
    fn from(value: LifecycleFilterArg) -> Self {
        match value {
            LifecycleFilterArg::Running => Self::Running,
            LifecycleFilterArg::Paused => Self::Paused,
            LifecycleFilterArg::Ended => Self::Ended,
        }
    }
}
