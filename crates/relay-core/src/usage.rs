//! Token usage, normalised so the categories never overlap.

use serde::{Deserialize, Serialize};
use std::ops::AddAssign;

/// Tokens for one request. The four billable buckets are disjoint, so
/// [`TokenUsage::total`] never double counts:
///
/// - `input`: uncached input. For OpenAI this is `input_tokens - cached_tokens`.
/// - `cache_read`: input served from the prompt cache.
/// - `cache_write`: input written to the cache (Anthropic only).
/// - `output`: all output, **including** reasoning.
///
/// `reasoning` is a subset of `output`, reported for display only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Uncached input tokens.
    pub input: u64,
    /// Cache-read input tokens.
    pub cache_read: u64,
    /// Cache-write input tokens.
    pub cache_write: u64,
    /// Output tokens including reasoning.
    pub output: u64,
    /// Reasoning tokens (subset of `output`).
    pub reasoning: u64,
}

impl TokenUsage {
    /// All input-side tokens.
    pub fn input_total(&self) -> u64 {
        self.input + self.cache_read + self.cache_write
    }

    /// Input plus output. This is the "Through Relay" measure.
    pub fn total(&self) -> u64 {
        self.input_total() + self.output
    }

    /// True when every bucket is zero.
    pub fn is_zero(&self) -> bool {
        *self == TokenUsage::default()
    }
}

impl AddAssign for TokenUsage {
    fn add_assign(&mut self, rhs: TokenUsage) {
        self.input += rhs.input;
        self.cache_read += rhs.cache_read;
        self.cache_write += rhs.cache_write;
        self.output += rhs.output;
        self.reasoning += rhs.reasoning;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_is_not_added_to_total() {
        let u = TokenUsage {
            input: 10,
            cache_read: 100,
            cache_write: 5,
            output: 40,
            reasoning: 30,
        };
        assert_eq!(u.total(), 155);
        assert_eq!(u.input_total(), 115);
    }
}
