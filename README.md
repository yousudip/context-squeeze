<div align="center">

# 🗜️ Context Squeeze

**A deterministic context-compression layer for Claude (and any MCP client).**

Squeeze codebases, files, and log streams down to the tokens that actually
matter — using parsers and tree-walking, not another expensive LLM call.

[![CI](https://github.com/yousudip/context-squeeze/actions/workflows/ci.yml/badge.svg)](https://github.com/yousudip/context-squeeze/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.82%2B-orange.svg)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-stdio-blue.svg)](https://modelcontextprotocol.io)

</div>

---

> [!WARNING]
> **Status: early development (pre-`0.1.0`).** The architecture and roadmap below
> are real and being built in the open. Follow [`docs/ROADMAP.md`](docs/ROADMAP.md)
> for live progress. Stars and early feedback are very welcome.

## The problem

Every token Claude spends *reading* a 4,000-line file or a 50,000-line log dump
is a token it can't spend *reasoning*. The usual fix — ask an LLM to summarize
the input first — is slow, costs money, is non-deterministic, and routinely
deletes the one variable or stack frame you actually needed.

## The idea

**Most of that bloat is mechanical, and machines can remove it deterministically.**

Comments, docstrings, blank-line padding, import ceremony, repeated stack traces,
timestamps, and ANSI noise carry little signal for a model that already
understands code. Context Squeeze sits between Claude and your raw data as an
[MCP](https://modelcontextprotocol.io) server, parses inputs with
[tree-sitter](https://tree-sitter.github.io/tree-sitter/), and emits a compact,
**syntactically faithful** projection that fits a token budget you choose.

```
 ┌────────────────┐   stdio / MCP    ┌──────────────────────┐        ┌─────────────────────┐
 │  Claude Desktop │◀───────────────▶│   Context Squeeze     │◀──────▶│  Filesystem / Git    │
 │  (or any client)│   JSON-RPC       │   MCP server (Rust)   │        │  source, docs, logs  │
 └────────────────┘                  └───────────┬──────────┘        └─────────────────────┘
                                                  │
                            ┌─────────────────────┼─────────────────────┐
                            ▼                     ▼                     ▼
                   ┌────────────────┐   ┌──────────────────┐   ┌──────────────────┐
                   │ tree-sitter AST│   │ Budget Allocator │   │  Log Distiller   │
                   │ strips fluff,  │   │ tiktoken-based   │   │ dedupes traces,  │
                   │ keeps structure│   │ collapse-to-fit  │   │ builds error map │
                   └────────────────┘   └──────────────────┘   └──────────────────┘
```

## The three tools

| MCP tool | What it does |
|---|---|
| **`inspect_codebase_skeleton(path)`** | Walks a directory (honoring `.gitignore`) and returns a condensed map of files with only their class/function/type **signatures** — the shape of a codebase at a fraction of the tokens. |
| **`fetch_squeezed_file(path, token_budget)`** | Reads one file, strips comments/docstrings/padding via the AST, then **progressively collapses function bodies to signatures** until the result fits `token_budget` — never breaking syntax. |
| **`summarize_log_stream(raw_text)`** | Collapses a 50,000-token log into a ~500-token **error anatomy**: timestamps stripped, duplicate stack traces folded with counts, repetitive lines clustered. |

## Why Rust

- **Native tree-sitter** — the C grammars compile straight in; no WASM, no FFI gymnastics.
- **Fast and lightweight** — a single static binary, ideal for a tiny local Docker image.
- **`#![forbid(unsafe_code)]`** across the workspace.

## Design principles

1. **Deterministic over clever.** Same input + same budget ⇒ byte-identical output. No model in the hot path.
2. **Context-preserving compression.** It is better to return slightly more than to silently drop the line that holds the bug. Squeezing degrades gracefully and signals what was elided.
3. **Local & private.** Your code and logs never leave the machine. No API key required.
4. **Honest budgets.** Token counts are an offline approximation (OpenAI `cl100k`), conservative by design. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md#tokenization).

## Quickstart

> Subcommands and the MCP server are landing phase-by-phase — see the
> [roadmap](docs/ROADMAP.md). Once the MVP ships:

```bash
# Build from source
cargo build --release

# Try the engine from the CLI
./target/release/cx skeleton ./my-project
./target/release/cx squeeze ./src/big_file.rs --budget 800

# Register the MCP server with Claude Desktop (see docs/USAGE.md)
```

## Repository layout

```
context-squeeze/
├── crates/
│   ├── cx-core/   # the compression engine (parsing, budgeting, squeezing) — all the logic
│   ├── cx-mcp/    # thin MCP (stdio) server wrapper exposing the three tools
│   └── cx-cli/    # thin CLI wrapper for local use, tests, and benchmarks
├── docs/          # architecture, project spec, roadmap, usage
└── .github/       # CI and contribution templates
```

## Documentation

- 📐 [**Architecture**](docs/ARCHITECTURE.md) — how the engine is put together
- 📋 [**Project spec**](docs/PROJECT.md) — vision, scope, goals, non-goals
- 🗺️ [**Roadmap**](docs/ROADMAP.md) — the fine-grained, phase-by-phase build plan
- 🤝 [**Contributing**](CONTRIBUTING.md) — how to get involved

## Contributing

This is built in the open and contributors are genuinely welcome — parser
edge cases, new language grammars, and budgeting heuristics are all great
entry points. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and the
[`good first issue`](https://github.com/yousudip/context-squeeze/labels/good%20first%20issue) label.

## License

[MIT](LICENSE) © Sudip Bhakta
