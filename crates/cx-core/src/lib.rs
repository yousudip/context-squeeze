//! # cx-core
//!
//! The deterministic context-compression engine behind **Context Squeeze**.
//!
//! `cx-core` turns large, noisy inputs (source trees, individual files, log
//! streams) into compact, structure-preserving representations that fit inside
//! a caller-supplied token budget — using ordinary software engineering
//! (parsing, tree walking, deduplication) rather than an LLM.
//!
//! The engine is transport-agnostic. The MCP server (`cx-mcp`) and the CLI
//! (`cx`) are thin wrappers over the functions exposed here.
//!
//! ## Module map
//!
//! | Module        | Responsibility                                            | Roadmap |
//! |---------------|-----------------------------------------------------------|---------|
//! | `tokenizer`   | Deterministic, offline token counting + budgeting         | Phase 1 |
//! | `ast`         | tree-sitter parsing and the supported-language registry   | Phase 2 |
//! | `skeleton`    | Codebase skeleton extraction (`inspect_codebase_skeleton`)| Phase 3 |
//! | `squeeze`     | Budget-driven single-file compression (`fetch_squeezed_file`) | Phase 4 |
//! | `logs`        | Log-stream summarization (`summarize_log_stream`)         | Phase 5 |
//!
//! Modules are introduced phase-by-phase; see `docs/ROADMAP.md`.

pub mod ast;
pub mod error;
pub mod skeleton;
pub mod squeeze;
pub mod tokenizer;

pub use ast::{Declaration, Language};
pub use error::{CxError, Result};
pub use skeleton::{codebase_skeleton, file_skeleton, SkeletonOptions, SkeletonReport};
pub use squeeze::{squeeze_file, SqueezeLevel, SqueezeResult};
pub use tokenizer::{Budget, Cl100kCounter, TokenCount, TokenCounter};

/// The crate version, surfaced by the CLI and MCP server handshake.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
