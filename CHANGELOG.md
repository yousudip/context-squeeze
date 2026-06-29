# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

The MVP is feature-complete on `main` and pending its first tagged release.

### Added
- **Engine (`cx-core`)** — deterministic, offline context compression:
  - Offline token counting (`cl100k`) and a conservative `Budget` allocator.
  - tree-sitter AST engine with a language registry (Python, JavaScript,
    TypeScript, TSX, Go, Rust) and a declaration walker.
  - Codebase skeleton extraction (`inspect_codebase_skeleton`).
  - Budget-driven single-file squeezing along a seven-level degradation ladder
    (`fetch_squeezed_file`), with a re-parse invariant on all code-valid levels.
  - Log-stream summarization (`summarize_log_stream`) with normalization,
    stack-trace folding, clustering, and severity ranking.
- **MCP server (`cx-mcp`)** — an `rmcp` 2.0 stdio server exposing the three tools
  to Claude Desktop and other MCP clients.
- **CLI (`cx`)** — `skeleton`, `squeeze`, and `logs` subcommands with human and
  `--json` output; `logs` reads from a file or stdin.
- Snapshot, golden, and invariant tests; a `criterion` benchmark; an `examples/`
  corpus; CI across Linux/macOS/Windows; a multi-stage Dockerfile; and a
  cross-platform release workflow.

[Unreleased]: https://github.com/yousudip/context-squeeze/commits/main
