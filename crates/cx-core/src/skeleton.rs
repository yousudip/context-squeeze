//! Codebase skeleton extraction — the engine behind `inspect_codebase_skeleton`.
//!
//! Walks a directory (honoring ignore files) and reduces each supported source
//! file to its declaration **signatures**, producing a compact, token-light map
//! of a project's shape. Function/type bodies are dropped; nested declarations
//! (methods, inner functions) are preserved and indented by nesting depth.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::ast::{self, Declaration, Language};
use crate::error::Result;
use crate::tokenizer::{TokenCount, TokenCounter};

/// Default ceiling for files we will parse; larger files are listed but skipped.
pub const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024;

/// Options controlling a codebase skeleton walk.
#[derive(Debug, Clone)]
pub struct SkeletonOptions {
    /// Files larger than this (in bytes) are listed but not parsed.
    pub max_file_bytes: u64,
    /// Whether to list files we skipped (unsupported language, too large, …).
    pub list_skipped: bool,
}

impl Default for SkeletonOptions {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_FILE_BYTES,
            list_skipped: true,
        }
    }
}

/// Why a file appears in the report the way it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// Parsed and skeletonized.
    Parsed,
    /// Extension not recognized as a supported language.
    Unsupported,
    /// Exceeded `max_file_bytes`.
    TooLarge,
    /// Could not be read as UTF-8 text.
    Unreadable,
}

/// One file's contribution to the skeleton.
#[derive(Debug, Clone)]
pub struct FileSkeleton {
    /// Path relative to the walk root, using `/` separators.
    pub path: String,
    /// Detected language, if any.
    pub language: Option<Language>,
    /// Status of this file in the report.
    pub status: FileStatus,
    /// The rendered skeleton lines (empty unless `status == Parsed`).
    pub skeleton: String,
}

/// The result of a codebase skeleton walk.
#[derive(Debug, Clone)]
pub struct SkeletonReport {
    /// Per-file entries, sorted by path for determinism.
    pub files: Vec<FileSkeleton>,
    /// The final, model-ready rendered text.
    pub rendered: String,
    /// Tokens in the original source of the parsed files.
    pub original_tokens: TokenCount,
    /// Tokens in the rendered skeleton.
    pub skeleton_tokens: TokenCount,
}

impl SkeletonReport {
    /// Number of files that were parsed and skeletonized.
    pub fn parsed_count(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.status == FileStatus::Parsed)
            .count()
    }

    /// Fraction of tokens removed vs. the original parsed source, in `[0, 1]`.
    /// Returns `0.0` when there was nothing to compress.
    pub fn reduction_ratio(&self) -> f32 {
        let original = self.original_tokens.get();
        if original == 0 {
            return 0.0;
        }
        let kept = self.skeleton_tokens.get().min(original);
        1.0 - (kept as f32 / original as f32)
    }
}

/// Collapse a declaration's header into a single, whitespace-normalized line.
fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Render a flat declaration list (in source order, pre-order DFS) into an
/// indented, signature-only skeleton.
fn render_decls(source: &str, decls: &[Declaration]) -> String {
    let mut out = String::new();
    for (i, decl) in decls.iter().enumerate() {
        let indent = "  ".repeat(decl.depth);
        let header = collapse_ws(&source[decl.header.clone()]);
        // In pre-order, a declaration has nested members iff the next entry is
        // one level deeper. Containers show their members; leaves show "…".
        let has_children = decls.get(i + 1).is_some_and(|n| n.depth == decl.depth + 1);
        out.push_str(&indent);
        out.push_str(&header);
        if !has_children && decl.has_body() {
            out.push_str(" …");
        }
        out.push('\n');
    }
    out
}

/// Produce the signature-only skeleton of a single source string.
pub fn file_skeleton(source: &str, language: Language) -> Result<String> {
    let tree = ast::parse(source, language)?;
    let decls = ast::declarations(&tree, source, language);
    Ok(render_decls(source, &decls))
}

/// Walk `root` and produce a codebase skeleton, measuring token reduction with
/// `counter`.
pub fn codebase_skeleton(
    root: &Path,
    counter: &impl TokenCounter,
    opts: &SkeletonOptions,
) -> Result<SkeletonReport> {
    // Collect file paths first, then sort, so output is deterministic regardless
    // of filesystem/walk ordering.
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in WalkBuilder::new(root).build() {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_some_and(|t| t.is_file()) {
            paths.push(entry.into_path());
        }
    }
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    let mut original_tokens = 0usize;

    for path in &paths {
        let rel = relative_path(root, path);
        let language = Language::from_path(path);

        let Some(language) = language else {
            files.push(FileSkeleton {
                path: rel,
                language: None,
                status: FileStatus::Unsupported,
                skeleton: String::new(),
            });
            continue;
        };

        let too_large = std::fs::metadata(path)
            .map(|m| m.len() > opts.max_file_bytes)
            .unwrap_or(false);
        if too_large {
            files.push(FileSkeleton {
                path: rel,
                language: Some(language),
                status: FileStatus::TooLarge,
                skeleton: String::new(),
            });
            continue;
        }

        let Ok(source) = std::fs::read_to_string(path) else {
            files.push(FileSkeleton {
                path: rel,
                language: Some(language),
                status: FileStatus::Unreadable,
                skeleton: String::new(),
            });
            continue;
        };

        original_tokens += counter.count(&source).get();
        let skeleton = file_skeleton(&source, language)?;
        files.push(FileSkeleton {
            path: rel,
            language: Some(language),
            status: FileStatus::Parsed,
            skeleton,
        });
    }

    let rendered = render_report(&files, opts);
    let skeleton_tokens = counter.count(&rendered);

    Ok(SkeletonReport {
        files,
        rendered,
        original_tokens: TokenCount(original_tokens),
        skeleton_tokens,
    })
}

/// Render the per-file entries into the final document.
fn render_report(files: &[FileSkeleton], opts: &SkeletonOptions) -> String {
    let mut out = String::new();
    for file in files {
        match file.status {
            FileStatus::Parsed => {
                let lang = file.language.map(|l| l.name()).unwrap_or("?");
                out.push_str(&format!("## {} [{}]\n", file.path, lang));
                if file.skeleton.trim().is_empty() {
                    out.push_str("(no top-level declarations)\n");
                } else {
                    out.push_str(&file.skeleton);
                }
                out.push('\n');
            }
            FileStatus::Unsupported | FileStatus::TooLarge | FileStatus::Unreadable => {
                if opts.list_skipped {
                    let reason = match file.status {
                        FileStatus::Unsupported => "unsupported",
                        FileStatus::TooLarge => "too large",
                        FileStatus::Unreadable => "unreadable",
                        FileStatus::Parsed => unreachable!(),
                    };
                    out.push_str(&format!("## {} [skipped: {}]\n\n", file.path, reason));
                }
            }
        }
    }
    // Trim the trailing blank line for a stable, clean tail.
    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

/// Compute a `/`-separated path relative to `root` (falling back to the file
/// name, then the full path, if stripping fails).
fn relative_path(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let rel = if rel.as_os_str().is_empty() {
        path.file_name().map(Path::new).unwrap_or(path)
    } else {
        rel
    };
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Cl100kCounter;

    #[test]
    fn rust_file_skeleton_drops_bodies_keeps_methods() {
        let src = r#"
/// A point in 2D space.
struct Point { x: f64, y: f64 }

impl Point {
    fn area(&self) -> f64 {
        self.x * self.y
    }
}

fn main() {
    println!("hi");
}
"#;
        let skel = file_skeleton(src, Language::Rust).unwrap();
        assert!(skel.contains("struct Point …"), "{skel}");
        assert!(skel.contains("impl Point"), "{skel}");
        assert!(skel.contains("  fn area(&self) -> f64 …"), "{skel}");
        assert!(skel.contains("fn main() …"), "{skel}");
        // The body contents must be gone.
        assert!(!skel.contains("self.x * self.y"), "{skel}");
        assert!(!skel.contains("println!"), "{skel}");
    }

    #[test]
    fn python_multiline_signature_is_collapsed() {
        let src = "def f(\n    a,\n    b,\n):\n    return a + b\n";
        let skel = file_skeleton(src, Language::Python).unwrap();
        assert!(skel.contains("def f( a, b, ): …"), "{skel}");
    }

    #[test]
    fn skeleton_is_much_smaller_than_source() {
        let src = include_str!("skeleton.rs");
        let counter = Cl100kCounter::new().unwrap();
        let skel = file_skeleton(src, Language::Rust).unwrap();
        let before = counter.count(src).get();
        let after = counter.count(&skel).get();
        assert!(
            after * 3 < before,
            "expected big reduction: {after} vs {before}"
        );
    }
}
