use clap::ValueEnum;

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub(super) enum ProviderKind {
    Openai,
    Vllm,
}

impl ProviderKind {
    pub(super) const fn as_str(&self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Vllm => "vllm",
        }
    }
}
