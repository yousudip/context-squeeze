//! The crate-wide error type and `Result` alias.
//!
//! Every fallible path in `cx-core` returns [`Result`], so wrappers (`cx-mcp`,
//! `cx-cli`) can translate a single error enum into their own medium.

use thiserror::Error;

/// Convenience alias for `Result<T, CxError>`.
pub type Result<T> = std::result::Result<T, CxError>;

/// Errors produced by the Context Squeeze engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CxError {
    /// An I/O failure while reading a file or walking a directory.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The tree-sitter parser failed to produce a tree for the given language.
    #[error("failed to parse {language} source")]
    Parse {
        /// The language we attempted to parse.
        language: &'static str,
    },

    /// The file's language could not be determined or is not supported.
    #[error("unsupported language for path: {0}")]
    UnsupportedLanguage(String),

    /// The tokenizer backend could not be constructed or used.
    #[error("tokenizer error: {0}")]
    Tokenizer(String),
}
