//! The tree-sitter AST engine: parsing and declaration discovery.
//!
//! This module turns source text into a syntax tree and walks it to enumerate
//! declarations (functions, classes, types, …) with the byte ranges that the
//! skeleton and squeeze passes operate on. It is the only place that touches
//! tree-sitter directly; everything downstream works with [`Declaration`]s and
//! byte ranges.

mod language;

use std::ops::Range;

use tree_sitter::{Node, Parser, Tree};

pub use language::{LangSpec, Language};

use crate::error::{CxError, Result};

/// Parse `source` as `language`, returning the syntax tree.
///
/// A tree is returned even when the source contains syntax errors (tree-sitter
/// is error-tolerant); use [`parses_cleanly`] when you need an error-free parse.
pub fn parse(source: &str, language: Language) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.ts_language())
        .map_err(|_| CxError::Parse {
            language: language.name(),
        })?;
    parser.parse(source, None).ok_or(CxError::Parse {
        language: language.name(),
    })
}

/// Whether `source` parses without any error nodes — the invariant we assert on
/// every reduced output so squeezing can never emit broken syntax.
pub fn parses_cleanly(source: &str, language: Language) -> bool {
    match parse(source, language) {
        Ok(tree) => !tree.root_node().has_error(),
        Err(_) => false,
    }
}

/// A single declaration discovered in a syntax tree, expressed as byte ranges
/// over the original source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// The tree-sitter node kind (e.g. `"function_item"`).
    pub kind: &'static str,
    /// The declaration's identifier, if the grammar exposes one.
    pub name: Option<String>,
    /// Declaration-nesting depth (0 = top level; methods inside a class are 1).
    pub depth: usize,
    /// The full byte range of the declaration node.
    pub node: Range<usize>,
    /// The "header" range: from the node start up to the body (or the whole
    /// node when there is no collapsible body).
    pub header: Range<usize>,
    /// The body range, if this declaration has a collapsible body.
    pub body: Option<Range<usize>>,
}

impl Declaration {
    /// Whether this declaration has a body that squeezing can collapse.
    pub fn has_body(&self) -> bool {
        self.body.is_some()
    }
}

/// Enumerate the declarations in `tree`, including nested ones (methods within
/// classes, functions within `impl` blocks), in source order.
pub fn declarations(tree: &Tree, source: &str, language: Language) -> Vec<Declaration> {
    let spec = language.spec();
    let mut out = Vec::new();
    collect(tree.root_node(), source, &spec, 0, &mut out);
    out
}

fn collect(node: Node, source: &str, spec: &LangSpec, depth: usize, out: &mut Vec<Declaration>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if spec.decl_kinds.contains(&child.kind()) {
            out.push(build_decl(child, source, spec, depth));
            // Recurse to find declarations nested inside this one.
            collect(child, source, spec, depth + 1, out);
        } else {
            collect(child, source, spec, depth, out);
        }
    }
}

fn build_decl(node: Node, source: &str, spec: &LangSpec, depth: usize) -> Declaration {
    let name = decl_name(node, source, spec);
    let body = node
        .child_by_field_name(spec.body_field)
        .map(|n| n.byte_range());
    let node_range = node.byte_range();
    let header = match &body {
        Some(b) => node_range.start..b.start,
        None => node_range.clone(),
    };
    Declaration {
        kind: node.kind(),
        name,
        depth,
        node: node_range,
        header,
        body,
    }
}

/// Resolve a declaration's name, falling back one level for grammars that nest
/// the identifier (e.g. Go's `type_declaration` → `type_spec.name`).
fn decl_name(node: Node, source: &str, spec: &LangSpec) -> Option<String> {
    if let Some(n) = node.child_by_field_name(spec.name_field) {
        return Some(source[n.byte_range()].to_string());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(n) = child.child_by_field_name(spec.name_field) {
            return Some(source[n.byte_range()].to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(source: &str, lang: Language) -> Vec<String> {
        let tree = parse(source, lang).expect("parse");
        declarations(&tree, source, lang)
            .into_iter()
            .filter_map(|d| d.name)
            .collect()
    }

    #[test]
    fn rust_declarations_including_methods() {
        let src = r#"
struct Point { x: f64, y: f64 }
impl Point {
    fn area(&self) -> f64 { self.x * self.y }
}
fn main() { let _ = Point { x: 1.0, y: 2.0 }; }
"#;
        let got = names(src, Language::Rust);
        assert!(got.contains(&"Point".to_string()), "got {got:?}");
        assert!(got.contains(&"area".to_string()), "got {got:?}");
        assert!(got.contains(&"main".to_string()), "got {got:?}");
    }

    #[test]
    fn python_class_method_and_function() {
        let src = "class Foo:\n    def bar(self):\n        return 1\n\ndef baz():\n    return 2\n";
        let got = names(src, Language::Python);
        assert!(got.contains(&"Foo".to_string()), "got {got:?}");
        assert!(got.contains(&"bar".to_string()), "got {got:?}");
        assert!(got.contains(&"baz".to_string()), "got {got:?}");
    }

    #[test]
    fn typescript_interface_class_and_function() {
        let src = r#"
interface Shape { area(): number; }
class Circle implements Shape {
    constructor(private r: number) {}
    area(): number { return Math.PI * this.r * this.r; }
}
function make(r: number): Circle { return new Circle(r); }
"#;
        let got = names(src, Language::TypeScript);
        assert!(got.contains(&"Shape".to_string()), "got {got:?}");
        assert!(got.contains(&"Circle".to_string()), "got {got:?}");
        assert!(got.contains(&"make".to_string()), "got {got:?}");
    }

    #[test]
    fn go_func_and_type_with_name_fallback() {
        let src = "package main\n\ntype Point struct {\n\tX int\n\tY int\n}\n\nfunc Add(a, b int) int {\n\treturn a + b\n}\n";
        let got = names(src, Language::Go);
        assert!(got.contains(&"Add".to_string()), "got {got:?}");
        // Name fallback resolves the nested type_spec identifier.
        assert!(got.contains(&"Point".to_string()), "got {got:?}");
    }

    #[test]
    fn function_body_range_is_captured() {
        let src = "fn add(a: i32) -> i32 { a + 1 }";
        let tree = parse(src, Language::Rust).unwrap();
        let decls = declarations(&tree, src, Language::Rust);
        let add = decls
            .iter()
            .find(|d| d.name.as_deref() == Some("add"))
            .unwrap();
        assert!(add.has_body());
        let body = add.body.clone().unwrap();
        assert_eq!(&src[body], "{ a + 1 }");
        assert_eq!(&src[add.header.clone()], "fn add(a: i32) -> i32 ");
    }

    #[test]
    fn method_depth_is_one() {
        let src = "impl T { fn m(&self) {} }";
        let tree = parse(src, Language::Rust).unwrap();
        let decls = declarations(&tree, src, Language::Rust);
        let m = decls
            .iter()
            .find(|d| d.name.as_deref() == Some("m"))
            .unwrap();
        assert_eq!(m.depth, 1);
    }

    #[test]
    fn clean_parse_detection() {
        assert!(parses_cleanly("fn ok() {}", Language::Rust));
        assert!(!parses_cleanly("fn broken( {", Language::Rust));
    }
}
