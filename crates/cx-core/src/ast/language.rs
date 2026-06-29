//! The supported-language registry.
//!
//! Adding a language is intentionally a small, self-contained change: add an
//! enum variant, wire its grammar in [`Language::ts_language`], map its file
//! extensions in [`Language::from_extension`], and describe its declaration and
//! comment node kinds in [`Language::spec`]. Everything else in the engine is
//! language-agnostic and reads from this one place.

/// A source language Context Squeeze can parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    /// TypeScript with JSX (`.tsx`).
    Tsx,
    Go,
    Rust,
}

/// Per-language node-kind metadata used by the skeleton and squeeze passes.
///
/// Field names (`name`, `body`) are tree-sitter *field* names that are stable
/// across our target grammars; node *kinds* are listed explicitly per language.
#[derive(Debug, Clone, Copy)]
pub struct LangSpec {
    /// Node kinds that represent declarations we want to surface (functions,
    /// classes, types, …).
    pub decl_kinds: &'static [&'static str],
    /// The subset of `decl_kinds` that are function-like — i.e. whose bodies the
    /// squeeze pass may collapse to a stub.
    pub fn_kinds: &'static [&'static str],
    /// The field name holding a declaration's identifier.
    pub name_field: &'static str,
    /// The field name holding a declaration's body (the part squeezing collapses).
    /// Declarations without this field (e.g. a type alias) are kept whole.
    pub body_field: &'static str,
    /// Node kinds that are comments (stripped by the squeeze pass).
    pub comment_kinds: &'static [&'static str],
}

impl Language {
    /// Detect the language from a file path's extension, if supported.
    pub fn from_path(path: &std::path::Path) -> Option<Language> {
        let ext = path.extension()?.to_str()?;
        Language::from_extension(ext)
    }

    /// Detect the language from a bare extension (without the dot).
    pub fn from_extension(ext: &str) -> Option<Language> {
        let lang = match ext {
            "py" | "pyi" => Language::Python,
            "js" | "mjs" | "cjs" | "jsx" => Language::JavaScript,
            "ts" | "mts" | "cts" => Language::TypeScript,
            "tsx" => Language::Tsx,
            "go" => Language::Go,
            "rs" => Language::Rust,
            _ => return None,
        };
        Some(lang)
    }

    /// A stable, human-readable name for the language.
    pub fn name(self) -> &'static str {
        match self {
            Language::Python => "Python",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Tsx => "TSX",
            Language::Go => "Go",
            Language::Rust => "Rust",
        }
    }

    /// The tree-sitter grammar for this language.
    pub fn ts_language(self) -> tree_sitter::Language {
        match self {
            Language::Python => tree_sitter_python::LANGUAGE.into(),
            Language::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Language::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Language::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Language::Go => tree_sitter_go::LANGUAGE.into(),
            Language::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    /// The declaration/comment metadata for this language.
    pub fn spec(self) -> LangSpec {
        match self {
            Language::Python => LangSpec {
                decl_kinds: &["function_definition", "class_definition"],
                fn_kinds: &["function_definition"],
                name_field: "name",
                body_field: "body",
                comment_kinds: &["comment"],
            },
            // TypeScript/TSX share JavaScript's declaration kinds plus the
            // TypeScript-only type constructs.
            Language::JavaScript => LangSpec {
                decl_kinds: &[
                    "function_declaration",
                    "generator_function_declaration",
                    "class_declaration",
                    "method_definition",
                ],
                fn_kinds: &[
                    "function_declaration",
                    "generator_function_declaration",
                    "method_definition",
                ],
                name_field: "name",
                body_field: "body",
                comment_kinds: &["comment"],
            },
            Language::TypeScript | Language::Tsx => LangSpec {
                decl_kinds: &[
                    "function_declaration",
                    "generator_function_declaration",
                    "class_declaration",
                    "abstract_class_declaration",
                    "method_definition",
                    "interface_declaration",
                    "type_alias_declaration",
                    "enum_declaration",
                ],
                fn_kinds: &[
                    "function_declaration",
                    "generator_function_declaration",
                    "method_definition",
                ],
                name_field: "name",
                body_field: "body",
                comment_kinds: &["comment"],
            },
            Language::Go => LangSpec {
                decl_kinds: &[
                    "function_declaration",
                    "method_declaration",
                    "type_declaration",
                ],
                fn_kinds: &["function_declaration", "method_declaration"],
                name_field: "name",
                body_field: "body",
                comment_kinds: &["comment"],
            },
            Language::Rust => LangSpec {
                decl_kinds: &[
                    "function_item",
                    "struct_item",
                    "enum_item",
                    "union_item",
                    "trait_item",
                    "impl_item",
                    "mod_item",
                    "type_item",
                    "macro_definition",
                ],
                fn_kinds: &["function_item"],
                name_field: "name",
                body_field: "body",
                comment_kinds: &["line_comment", "block_comment"],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn detects_languages_by_extension() {
        assert_eq!(Language::from_extension("py"), Some(Language::Python));
        assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
        assert_eq!(Language::from_extension("ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_extension("tsx"), Some(Language::Tsx));
        assert_eq!(Language::from_extension("jsx"), Some(Language::JavaScript));
        assert_eq!(Language::from_extension("go"), Some(Language::Go));
        assert_eq!(Language::from_extension("txt"), None);
    }

    #[test]
    fn detects_language_from_path() {
        assert_eq!(
            Language::from_path(Path::new("a/b/c.py")),
            Some(Language::Python)
        );
        assert_eq!(Language::from_path(Path::new("Cargo.toml")), None);
        assert_eq!(Language::from_path(Path::new("noext")), None);
    }

    #[test]
    fn every_grammar_loads() {
        // Catches grammar/ABI mismatches at test time for every language.
        for lang in [
            Language::Python,
            Language::JavaScript,
            Language::TypeScript,
            Language::Tsx,
            Language::Go,
            Language::Rust,
        ] {
            let mut parser = tree_sitter::Parser::new();
            parser
                .set_language(&lang.ts_language())
                .unwrap_or_else(|e| panic!("{} grammar failed to load: {e}", lang.name()));
        }
    }
}
