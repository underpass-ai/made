//! [`TokenUsage`] — prompt + completion token counts for one LLM call.
//!
//! A small, immutable record of what a single model call cost in tokens.
//! Naming the two counts as one value object keeps the metric port free
//! of bare `u32` pairs that are trivial to transpose.

/// Token counts reported by a model for a single call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenUsage {
    prompt: u32,
    completion: u32,
}

impl TokenUsage {
    /// Build a usage record from prompt (input) and completion (output)
    /// token counts.
    #[must_use]
    pub const fn new(prompt: u32, completion: u32) -> Self {
        Self { prompt, completion }
    }

    /// Tokens consumed by the request (the "prompt" / "input" side).
    #[must_use]
    pub const fn prompt(self) -> u32 {
        self.prompt
    }

    /// Tokens produced in the response (the "completion" / "output" side).
    #[must_use]
    pub const fn completion(self) -> u32 {
        self.completion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessors_return_constructed_counts() {
        let usage = TokenUsage::new(120, 45);
        assert_eq!(usage.prompt(), 120);
        assert_eq!(usage.completion(), 45);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(TokenUsage::default(), TokenUsage::new(0, 0));
    }
}
