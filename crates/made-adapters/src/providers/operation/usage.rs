use made_core::value_objects::TokenUsage;

/// Usage counts; `None` means the source did not report that dimension, not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    total_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

impl Usage {
    #[must_use]
    pub const fn new(input_tokens: Option<u64>, output_tokens: Option<u64>) -> Self {
        Self {
            input_tokens,
            output_tokens,
            total_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
        }
    }
    #[must_use]
    pub const fn with_total(mut self, total_tokens: u64) -> Self {
        self.total_tokens = Some(total_tokens);
        self
    }
    #[must_use]
    pub const fn with_cache_read(mut self, tokens: u64) -> Self {
        self.cache_read_tokens = Some(tokens);
        self
    }
    #[must_use]
    pub const fn with_cache_write(mut self, tokens: u64) -> Self {
        self.cache_write_tokens = Some(tokens);
        self
    }
    #[must_use]
    pub const fn input_tokens(self) -> Option<u64> {
        self.input_tokens
    }
    #[must_use]
    pub const fn output_tokens(self) -> Option<u64> {
        self.output_tokens
    }
    #[must_use]
    pub const fn total_tokens(self) -> Option<u64> {
        self.total_tokens
    }
    #[must_use]
    pub const fn cache_read_tokens(self) -> Option<u64> {
        self.cache_read_tokens
    }
    #[must_use]
    pub const fn cache_write_tokens(self) -> Option<u64> {
        self.cache_write_tokens
    }
}

impl From<TokenUsage> for Usage {
    fn from(value: TokenUsage) -> Self {
        Self::new(
            Some(u64::from(value.prompt())),
            Some(u64::from(value.completion())),
        )
    }
}
