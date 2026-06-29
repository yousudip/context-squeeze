//! Budget-driven single-file compression — the engine behind `fetch_squeezed_file`.
//!
//! Squeezing applies an ordered **degradation ladder** to a file and returns the
//! richest level that fits the requested [`Budget`]. Reductions are expressed as
//! byte-range edits over the *original* source (we never pretty-print an AST
//! back to text), so the code-preserving levels remain verbatim original code
//! and always re-parse.
//!
//! ## The ladder
//!
//! | Level | Name | Valid source? | What it does |
//! |-------|------|---------------|--------------|
//! | L0 | [`Verbatim`](SqueezeLevel::Verbatim) | yes | the original file |
//! | L1 | [`NoComments`](SqueezeLevel::NoComments) | yes | strip comments + safe docstrings |
//! | L2 | [`NoPadding`](SqueezeLevel::NoPadding) | yes | + collapse blank/trailing whitespace |
//! | L3 | [`PartialCollapse`](SqueezeLevel::PartialCollapse) | yes | + collapse the largest function bodies to stubs |
//! | L4 | [`FullCollapse`](SqueezeLevel::FullCollapse) | yes | + collapse *all* function bodies |
//! | L5 | [`Skeleton`](SqueezeLevel::Skeleton) | no | signature-only outline |
//! | L6 | [`TruncatedSkeleton`](SqueezeLevel::TruncatedSkeleton) | no | skeleton trimmed with an elision marker |
//!
//! Levels L0–L4 are guaranteed to re-parse without error; L5/L6 are compact
//! outlines (marked `is_valid_source = false`).

use std::fmt;
use std::ops::Range;

use crate::ast::{self, Language};
use crate::error::Result;
use crate::skeleton::file_skeleton;
use crate::tokenizer::{Budget, TokenCount, TokenCounter};

/// A rung on the degradation ladder. See the [module docs](self) for the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqueezeLevel {
    Verbatim,
    NoComments,
    NoPadding,
    PartialCollapse,
    FullCollapse,
    Skeleton,
    TruncatedSkeleton,
}

impl SqueezeLevel {
    /// Whether output at this level is valid, re-parseable source.
    pub fn is_valid_source(self) -> bool {
        !matches!(
            self,
            SqueezeLevel::Skeleton | SqueezeLevel::TruncatedSkeleton
        )
    }

    /// A short, human-readable description.
    pub fn description(self) -> &'static str {
        match self {
            SqueezeLevel::Verbatim => "verbatim",
            SqueezeLevel::NoComments => "comments and docstrings stripped",
            SqueezeLevel::NoPadding => "comments stripped, padding collapsed",
            SqueezeLevel::PartialCollapse => "largest function bodies collapsed to stubs",
            SqueezeLevel::FullCollapse => "all function bodies collapsed to stubs",
            SqueezeLevel::Skeleton => "signature-only skeleton",
            SqueezeLevel::TruncatedSkeleton => "truncated skeleton (declarations elided)",
        }
    }
}

impl fmt::Display for SqueezeLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// The outcome of squeezing a file.
#[derive(Debug, Clone)]
pub struct SqueezeResult {
    /// The ladder level the output was produced at.
    pub level: SqueezeLevel,
    /// The compressed output.
    pub output: String,
    /// Tokens in the original file.
    pub original_tokens: TokenCount,
    /// Tokens in `output`.
    pub output_tokens: TokenCount,
    /// Whether `output` is valid, re-parseable source (true for L0–L4).
    pub is_valid_source: bool,
    /// Number of function bodies collapsed (0 unless L3/L4).
    pub bodies_collapsed: usize,
    /// Whether `output` fits the requested budget. Only false when even the
    /// truncated skeleton overflows an extremely small budget.
    pub fits_budget: bool,
}

impl SqueezeResult {
    /// Fraction of tokens removed vs. the original, in `[0, 1]`.
    pub fn reduction_ratio(&self) -> f32 {
        let original = self.original_tokens.get();
        if original == 0 {
            return 0.0;
        }
        let kept = self.output_tokens.get().min(original);
        1.0 - (kept as f32 / original as f32)
    }
}

/// A single byte-range substitution over the original source.
#[derive(Debug, Clone)]
struct Edit {
    range: Range<usize>,
    replacement: String,
}

/// Apply edits to `source` in one pass. Edits are sorted by start; any edit that
/// overlaps an already-applied one is skipped (so collapsing an outer body
/// safely subsumes edits inside it).
fn apply_edits(source: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|e| e.range.start);
    let mut out = String::with_capacity(source.len());
    let mut pos = 0usize;
    for edit in edits {
        if edit.range.start < pos {
            continue; // overlaps a prior edit
        }
        out.push_str(&source[pos..edit.range.start]);
        out.push_str(&edit.replacement);
        pos = edit.range.end;
    }
    out.push_str(&source[pos..]);
    out
}

/// Normalize whitespace: trim trailing spaces and collapse runs of blank lines
/// to a single blank line.
fn normalize_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut blank_run = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out
}

/// The stub a collapsed function body is replaced with, per language.
fn body_placeholder(language: Language) -> &'static str {
    match language {
        // The block/suite is replaced by an Ellipsis statement; the header range
        // already carries the colon and indentation, so this stays valid.
        Language::Python => "...",
        // Brace languages keep an (empty) block containing only a marker comment.
        _ => "{ /* … */ }",
    }
}

/// Drop ranges fully contained within another range (keeps the outermost), so a
/// collapsed outer body isn't double-counted with its nested functions.
fn drop_contained(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut out: Vec<Range<usize>> = Vec::new();
    for r in ranges {
        if let Some(last) = out.last() {
            if r.start >= last.start && r.end <= last.end {
                continue;
            }
        }
        out.push(r);
    }
    out
}

/// Squeeze `source` to fit `budget`, returning the richest level that fits.
pub fn squeeze_file(
    source: &str,
    language: Language,
    budget: Budget,
    counter: &impl TokenCounter,
) -> Result<SqueezeResult> {
    let original_tokens = counter.count(source);

    let finalize = |level: SqueezeLevel, output: String, collapsed: usize| -> SqueezeResult {
        let output_tokens = counter.count(&output);
        SqueezeResult {
            level,
            is_valid_source: level.is_valid_source(),
            fits_budget: budget.fits(output_tokens),
            output,
            original_tokens,
            output_tokens,
            bodies_collapsed: collapsed,
        }
    };

    // L0 — verbatim.
    if budget.fits(original_tokens) {
        return Ok(finalize(SqueezeLevel::Verbatim, source.to_string(), 0));
    }

    let tree = ast::parse(source, language)?;

    // Edits that strip every comment and every safe docstring.
    let comment_edits: Vec<Edit> = ast::comment_ranges(&tree, language)
        .into_iter()
        .chain(ast::docstring_ranges(&tree, language))
        .map(|range| Edit {
            range,
            replacement: String::new(),
        })
        .collect();

    // L1 — strip comments/docstrings.
    let l1 = apply_edits(source, comment_edits.clone());
    if budget.fits(counter.count(&l1)) {
        return Ok(finalize(SqueezeLevel::NoComments, l1, 0));
    }

    // L2 — collapse padding.
    let l2 = normalize_ws(&l1);
    if budget.fits(counter.count(&l2)) {
        return Ok(finalize(SqueezeLevel::NoPadding, l2, 0));
    }

    // L3/L4 — collapse function bodies, largest first.
    let spec = language.spec();
    let decls = ast::declarations(&tree, source, language);
    let mut bodies: Vec<Range<usize>> = decls
        .iter()
        .filter(|d| spec.fn_kinds.contains(&d.kind) && d.has_body())
        .filter_map(|d| d.body.clone())
        .collect();
    bodies = drop_contained(bodies);
    bodies.sort_by_key(|r| std::cmp::Reverse(r.len()));

    let placeholder = body_placeholder(language);
    for take in 1..=bodies.len() {
        let mut edits = comment_edits.clone();
        for body in &bodies[..take] {
            edits.push(Edit {
                range: body.clone(),
                replacement: placeholder.to_string(),
            });
        }
        let text = normalize_ws(&apply_edits(source, edits));
        if budget.fits(counter.count(&text)) {
            let level = if take == bodies.len() {
                SqueezeLevel::FullCollapse
            } else {
                SqueezeLevel::PartialCollapse
            };
            return Ok(finalize(level, text, take));
        }
    }

    // L5 — signature-only skeleton (not valid source).
    let l5 = file_skeleton(source, language)?;
    if budget.fits(counter.count(&l5)) {
        return Ok(finalize(SqueezeLevel::Skeleton, l5, bodies.len()));
    }

    // L6 — truncated skeleton with an explicit elision marker.
    let l6 = truncate_to_budget(&l5, budget, counter);
    Ok(finalize(SqueezeLevel::TruncatedSkeleton, l6, bodies.len()))
}

/// Keep as many leading skeleton lines as fit the budget, then append a marker
/// noting how many declarations were dropped.
fn truncate_to_budget(skeleton: &str, budget: Budget, counter: &impl TokenCounter) -> String {
    let lines: Vec<&str> = skeleton.lines().collect();
    let mut kept = String::new();
    let mut elided = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let candidate = format!("{kept}{line}\n");
        if budget.fits(counter.count(&candidate)) {
            kept = candidate;
        } else {
            elided = lines.len() - i;
            break;
        }
    }
    if elided > 0 {
        kept.push_str(&format!("… [{elided} more declarations elided]\n"));
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::parses_cleanly;
    use crate::tokenizer::Cl100kCounter;

    const RUST_SAMPLE: &str = r#"
// A module-level comment.
use std::collections::HashMap;

/// Adds two numbers together.
fn add(a: i32, b: i32) -> i32 {
    // inline comment
    let total = a + b;
    total
}

/// Builds a frequency map from words.
fn frequencies(words: &[&str]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for w in words {
        *map.entry(w.to_string()).or_insert(0) += 1;
    }
    map
}

struct Config {
    name: String,
    retries: u32,
}
"#;

    const PYTHON_SAMPLE: &str = "\"\"\"Module docstring.\"\"\"\n\n\ndef greet(name):\n    \"\"\"Return a greeting.\"\"\"\n    # build the message\n    msg = f\"hello {name}\"\n    return msg\n\n\ndef farewell(name):\n    return f\"bye {name}\"\n";

    fn counter() -> Cl100kCounter {
        Cl100kCounter::new().unwrap()
    }

    #[test]
    fn verbatim_when_budget_is_generous() {
        let c = counter();
        let res = squeeze_file(RUST_SAMPLE, Language::Rust, Budget::new(100_000), &c).unwrap();
        assert_eq!(res.level, SqueezeLevel::Verbatim);
        assert_eq!(res.output, RUST_SAMPLE);
    }

    #[test]
    fn comments_are_stripped_at_l1() {
        let c = counter();
        // A budget just below the original but above the comment-stripped size.
        let original = c.count(RUST_SAMPLE).get();
        let res = squeeze_file(RUST_SAMPLE, Language::Rust, Budget::new(original), &c).unwrap();
        assert_ne!(res.level, SqueezeLevel::Verbatim);
        assert!(!res.output.contains("inline comment"), "{}", res.output);
        assert!(
            !res.output.contains("A module-level comment"),
            "{}",
            res.output
        );
    }

    #[test]
    fn tight_budget_collapses_bodies_but_stays_valid() {
        let c = counter();
        let res = squeeze_file(RUST_SAMPLE, Language::Rust, Budget::new(60), &c).unwrap();
        // Function signatures survive even though bodies are gone.
        assert!(
            res.output.contains("fn add(a: i32, b: i32) -> i32"),
            "{}",
            res.output
        );
        assert!(!res.output.contains("let total"), "{}", res.output);
        if res.is_valid_source {
            assert!(
                parses_cleanly(&res.output, Language::Rust),
                "invalid:\n{}",
                res.output
            );
        }
        assert!(res.fits_budget, "did not fit:\n{}", res.output);
    }

    #[test]
    fn python_docstrings_stripped_safely() {
        let c = counter();
        let original = c.count(PYTHON_SAMPLE).get();
        let res = squeeze_file(PYTHON_SAMPLE, Language::Python, Budget::new(original), &c).unwrap();
        // The greet docstring has a following statement, so it is removed; the
        // result must still parse.
        assert!(!res.output.contains("Return a greeting"), "{}", res.output);
        assert!(
            parses_cleanly(&res.output, Language::Python),
            "invalid:\n{}",
            res.output
        );
    }

    #[test]
    fn reparse_invariant_holds_across_languages_and_budgets() {
        let c = counter();
        let cases = [
            (RUST_SAMPLE, Language::Rust),
            (PYTHON_SAMPLE, Language::Python),
        ];
        for (src, lang) in cases {
            for limit in [40usize, 80, 200, 100_000] {
                let res = squeeze_file(src, lang, Budget::new(limit), &c).unwrap();
                assert!(
                    res.output_tokens.get() <= res.original_tokens.get(),
                    "{lang:?}@{limit}: output grew"
                );
                if res.is_valid_source {
                    assert!(
                        parses_cleanly(&res.output, lang),
                        "{lang:?}@{limit} produced invalid source:\n{}",
                        res.output
                    );
                }
            }
        }
    }

    #[test]
    fn very_tight_budget_truncates_skeleton() {
        let c = counter();
        let res = squeeze_file(RUST_SAMPLE, Language::Rust, Budget::new(12), &c).unwrap();
        assert!(matches!(
            res.level,
            SqueezeLevel::Skeleton | SqueezeLevel::TruncatedSkeleton
        ));
    }
}
