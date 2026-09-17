use clap::ValueEnum;

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub(super) enum ChainSelector {
    One,
    Two,
    #[value(name = "positive-path", alias = "positive", alias = "three")]
    PositivePath,
    All,
}
