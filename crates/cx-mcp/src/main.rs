//! Context Squeeze — MCP (Model Context Protocol) stdio server.
//!
//! This binary will expose the Context Squeeze tools to Claude Desktop and
//! other MCP clients over stdio. The `rmcp` wiring lands in Phase 6
//! (see `docs/ROADMAP.md`); until then this is an intentional placeholder so
//! the workspace builds end-to-end.

fn main() {
    eprintln!(
        "cx-mcp {} — MCP server not yet wired (Phase 6). See docs/ROADMAP.md.",
        cx_core::VERSION
    );
}
