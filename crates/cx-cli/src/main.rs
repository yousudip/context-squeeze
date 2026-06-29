//! Context Squeeze — developer CLI (`cx`).
//!
//! Mirrors the three MCP tools as terminal subcommands so the compression
//! engine can be exercised, golden-tested, and benchmarked without an MCP
//! client. Subcommands are implemented in Phase 7 (see `docs/ROADMAP.md`).

fn main() {
    println!(
        "cx {} — CLI subcommands land in Phase 7. See docs/ROADMAP.md.",
        cx_core::VERSION
    );
}
