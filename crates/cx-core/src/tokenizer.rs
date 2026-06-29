//! Deterministic, offline token counting and the [`Budget`] primitive.
//!
//! Counting is abstracted behind the [`TokenCounter`] trait so the backend can
//! be swapped (e.g. for an exact Anthropic adapter) without touching callers.
//! The default backend, [`Cl100kCounter`], uses OpenAI's `cl100k_base` BPE via
//! `tiktoken-rs`.
//!
//! ## A deliberate caveat
//!
//! `cl100k` is **not** Claude's tokenizer — it is a fast, fully-offline
//! *approximation*. To keep that approximation safe rather than optimistic, the
//! conservative bias lives in [`Budget`]: its [`effective_limit`] reserves a
//! safety margin below the caller's requested limit, so a "fits" answer has
//! headroom against the cl100k-vs-Claude discrepancy.
//!
//! [`effective_limit`]: Budget::effective_limit

use std::fmt;

use tiktoken_rs::CoreBPE;

use crate::error::{CxError, Result};

/// A count of tokens. A thin newtype so token counts can't be confused with
/// byte lengths, char counts, or budgets elsewhere in the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TokenCount(pub usize);

impl TokenCount {
    /// Zero tokens.
    pub const ZERO: TokenCount = TokenCount(0);

    /// The raw count as a `usize`.
    #[inline]
    pub const fn get(self) -> usize {
        self.0
    }

    /// Saturating subtraction; never underflows below zero.
    #[inline]
    pub fn saturating_sub(self, other: TokenCount) -> TokenCount {
        TokenCount(self.0.saturating_sub(other.0))
    }
}

impl fmt::Display for TokenCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for TokenCount {
    fn from(n: usize) -> Self {
        TokenCount(n)
    }
}

/// Something that can deterministically count the tokens in a string.
///
/// Implementations must be pure: the same input always yields the same count.
pub trait TokenCounter {
    /// Count the tokens in `text`.
    fn count(&self, text: &str) -> TokenCount;

    /// Convenience: does `text` fit within `budget`'s effective limit?
    fn fits(&self, text: &str, budget: &Budget) -> bool {
        budget.fits(self.count(text))
    }
}

/// The default token counter, backed by `cl100k_base`.
pub struct Cl100kCounter {
    bpe: CoreBPE,
}

impl Cl100kCounter {
    /// Construct the counter, loading the `cl100k_base` ranks (bundled in
    /// `tiktoken-rs`, so this performs no network I/O).
    pub fn new() -> Result<Self> {
        let bpe = tiktoken_rs::cl100k_base()
            .map_err(|e| CxError::Tokenizer(format!("failed to load cl100k_base: {e}")))?;
        Ok(Self { bpe })
    }
}

impl TokenCounter for Cl100kCounter {
    fn count(&self, text: &str) -> TokenCount {
        // `encode_ordinary` ignores special-token markers, which is what we want
        // for measuring arbitrary source/log text.
        TokenCount(self.bpe.encode_ordinary(text).len())
    }
}

// `CoreBPE` does not implement `Debug`; provide a minimal one so the workspace
// `missing_debug_implementations` lint stays satisfied.
impl fmt::Debug for Cl100kCounter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cl100kCounter").finish_non_exhaustive()
    }
}

/// The default fraction of a budget held back as safety headroom (10%).
pub const DEFAULT_SAFETY_MARGIN: f32 = 0.10;

/// A token budget: a caller-requested ceiling plus a conservative safety margin.
///
/// Callers ask for a `limit` (e.g. "fit in 800 tokens"). Because our token
/// counts are an approximation, [`fits`](Budget::fits) compares against an
/// [`effective_limit`](Budget::effective_limit) that sits a `safety_margin`
/// fraction below the requested limit. This biases decisions toward producing
/// slightly-smaller output rather than overshooting Claude's real window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Budget {
    limit: TokenCount,
    safety_margin: f32,
}

impl Budget {
    /// A budget for `limit` tokens using the [`DEFAULT_SAFETY_MARGIN`].
    pub fn new(limit: usize) -> Self {
        Self {
            limit: TokenCount(limit),
            safety_margin: DEFAULT_SAFETY_MARGIN,
        }
    }

    /// A budget with an explicit safety margin in `[0.0, 1.0)`. Out-of-range
    /// values are clamped to that interval.
    pub fn with_margin(limit: usize, safety_margin: f32) -> Self {
        let safety_margin = safety_margin.clamp(0.0, 0.99);
        Self {
            limit: TokenCount(limit),
            safety_margin,
        }
    }

    /// The caller's requested hard ceiling.
    #[inline]
    pub fn limit(self) -> TokenCount {
        self.limit
    }

    /// The safety margin fraction held back from the limit.
    #[inline]
    pub fn safety_margin(self) -> f32 {
        self.safety_margin
    }

    /// The limit actually used for fit decisions: `floor(limit * (1 - margin))`.
    pub fn effective_limit(self) -> TokenCount {
        let scaled = (self.limit.0 as f32 * (1.0 - self.safety_margin)).floor();
        TokenCount(scaled as usize)
    }

    /// Whether `count` fits within the effective limit.
    #[inline]
    pub fn fits(self, count: TokenCount) -> bool {
        count <= self.effective_limit()
    }

    /// Headroom remaining under the effective limit (zero if already over).
    #[inline]
    pub fn remaining(self, count: TokenCount) -> TokenCount {
        self.effective_limit().saturating_sub(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counter() -> Cl100kCounter {
        Cl100kCounter::new().expect("cl100k_base should load")
    }

    #[test]
    fn empty_string_is_zero_tokens() {
        assert_eq!(counter().count(""), TokenCount(0));
    }

    #[test]
    fn counting_is_deterministic() {
        let c = counter();
        let text = "fn add(a: i32, b: i32) -> i32 { a + b }";
        assert_eq!(c.count(text), c.count(text));
    }

    #[test]
    fn known_snippet_count_is_stable() {
        // Pinned reference value for cl100k_base; guards against accidental
        // backend/config drift.
        let c = counter();
        assert_eq!(
            c.count("fn add(a: i32, b: i32) -> i32 { a + b }"),
            TokenCount(20)
        );
    }

    #[test]
    fn more_text_means_more_tokens() {
        let c = counter();
        let short = c.count("hello");
        let long = c.count("hello hello hello hello hello");
        assert!(long > short, "{long} should exceed {short}");
    }

    #[test]
    fn budget_reserves_safety_margin() {
        let b = Budget::with_margin(100, 0.10);
        assert_eq!(b.limit(), TokenCount(100));
        assert_eq!(b.effective_limit(), TokenCount(90));
        assert!(b.fits(TokenCount(90)));
        assert!(!b.fits(TokenCount(91)));
    }

    #[test]
    fn budget_remaining_is_saturating() {
        let b = Budget::with_margin(100, 0.10); // effective 90
        assert_eq!(b.remaining(TokenCount(30)), TokenCount(60));
        assert_eq!(b.remaining(TokenCount(200)), TokenCount(0));
    }

    #[test]
    fn margin_is_clamped() {
        // A nonsensical margin is clamped, never panics or inverts the budget.
        let b = Budget::with_margin(100, 5.0);
        assert!(b.effective_limit() <= b.limit());
        let z = Budget::with_margin(100, -1.0);
        assert_eq!(z.effective_limit(), TokenCount(100));
    }

    #[test]
    fn counter_fits_helper_matches_budget() {
        let c = counter();
        let b = Budget::new(10_000);
        assert!(c.fits("a small string", &b));
    }
}
