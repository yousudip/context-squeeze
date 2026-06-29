# Context Squeeze — Architecture

This document explains *how* Context Squeeze is built. For scope and intent see
[`PROJECT.md`](PROJECT.md); for the build sequence see [`ROADMAP.md`](ROADMAP.md).

## 1. Shape of the system

Context Squeeze is a Cargo workspace with a **pure engine** and two **thin
wrappers**, so the compression logic is transport-independent and directly
testable.

```
                         ┌──────────────────────────────────────────┐
   MCP client (Claude) ──┤  cx-mcp   (rmcp stdio server)            │
                         └───────────────┬──────────────────────────┘
                                         │  calls
   Terminal user ──────► cx-cli  ────────┤
                                         ▼
                         ┌──────────────────────────────────────────┐
                         │  cx-core  (the engine — no I/O transport) │
                         │                                          │
                         │   tokenizer ──┐                          │
                         │      ast ─────┼─► skeleton                │
                         │               ├─► squeeze                 │
                         │               └─► logs                    │
                         └──────────────────────────────────────────┘
```

- **`cx-core`** — all logic. Parsing, tokenization, budgeting, squeezing,
  log distillation. Depends on no transport and does its own filesystem reads
  only through small, injectable seams (so it stays unit-testable on strings).
- **`cx-mcp`** — maps MCP tool calls ↔ `cx-core` functions over stdio via `rmcp`.
- **`cx-cli`** — maps terminal subcommands ↔ the same `cx-core` functions.

Both wrappers are intentionally dumb: argument parsing, error formatting, and
serialization only. No business logic lives in a wrapper.

## 2. Module responsibilities (`cx-core`)

| Module | Responsibility |
|---|---|
| `tokenizer` | Offline token counting and the `Budget` type. Wraps `tiktoken-rs` (`cl100k_base`) behind a `TokenCounter` trait so the backend is swappable. |
| `ast` | tree-sitter `Parser` management, the `Language` registry, file→language detection, and helpers to query declaration nodes per language. |
| `skeleton` | Directory walking (via `ignore`) and per-file reduction to signatures. Implements `inspect_codebase_skeleton`. |
| `squeeze` | The single-file **degradation ladder** and the budget-fitting loop. Implements `fetch_squeezed_file`. |
| `logs` | Line normalization, dedup, stack-trace folding, error clustering. Implements `summarize_log_stream`. |
| `error` | The crate's `CxError` type (via `thiserror`) and `Result` alias. |

## 3. The AST engine

tree-sitter parses each source file into a concrete syntax tree. We then walk the
tree and keep only the nodes that carry structural signal.

- **Language registry.** A single `Language` enum centralizes every supported
  grammar, its file extensions, and the node-kind names that denote
  *declarations* and *bodies*. Adding a language is one match arm plus a grammar
  crate — this is deliberately the lowest-friction extension point.
- **Declaration vs. body.** For each language we identify the node kinds that are
  "headers" (e.g. a function's name + parameters + return type) and the child
  node that is the "body" (e.g. the block). Squeezing replaces a body span with a
  placeholder (`…` / language-appropriate stub) while leaving the header intact.
- **Byte-span surgery.** Reductions are expressed as a set of *byte ranges to
  drop or replace* over the original source, then applied in a single pass. This
  keeps the transform total, ordered, and trivially deterministic.

### Why concrete spans, not re-printing
We never pretty-print an AST back to source (which would reformat and risk
semantic drift). We only **delete or substitute byte ranges** of the original
text. What survives is verbatim original code, so output re-parses cleanly and
diffs sanely.

## 4. Tokenization

> **The honest caveat.** `tiktoken` is OpenAI's tokenizer, not Anthropic's.
> Context Squeeze uses `cl100k_base` as a fast, fully-offline **approximation**
> of Claude's token counts.

- The `TokenCounter` trait abstracts counting so the backend can change without
  touching callers. The default impl is `cl100k_base`.
- Estimates are tuned to be **conservative** (bias toward over-counting) so a
  "fits the budget" result is safe rather than optimistic.
- An optional exact-count adapter (Anthropic's `count_tokens` API) is an
  explicit non-goal for `0.1.0` but the trait seam is there for it later.

## 5. The degradation ladder (single-file squeeze)

`fetch_squeezed_file` applies increasingly aggressive reductions and stops at the
**first level that fits** the requested `token_budget`, returning the richest
representation that still fits:

```
L0  Verbatim (original file)
L1  Strip comments + docstrings
L2  + collapse blank-line / formatting padding
L3  + collapse the largest function bodies to signatures (greedy, by token cost)
L4  + collapse ALL function bodies to signatures
L5  Header-only skeleton (declarations + types, no bodies)
L6  Truncated skeleton with an explicit "[N declarations elided]" marker
```

Each level is a pure function `source → reduced source`. The loop counts tokens
after each level and returns the first passing level together with metadata: the
level reached, original vs. final token counts, and what was elided. **Level 5 is
the floor for "show me the code"**; we never chop a body mid-statement to hit a
number — if even L5 overflows, L6 drops whole declarations and says so.

## 6. Log distillation

`summarize_log_stream` is a streaming line pipeline:

1. **Normalize** — strip ANSI, ISO/epoch timestamps, and high-cardinality tokens
   (UUIDs, hex addresses, line:col in temp paths) to a canonical form, keeping a
   representative original.
2. **Fold stack traces** — detect multi-line traces (language-aware prefixes like
   `at `, `File "...", line`, `goroutine`, `thread '...' panicked`) and treat each
   trace as a single unit keyed by its normalized signature.
3. **Cluster** — group normalized lines/traces by signature; count occurrences;
   record first/last ordinal.
4. **Emit error anatomy** — distinct error signatures ranked by severity then
   frequency, each with a count, its first representative, and a folded trace.

The result is a small, stable map of "what went wrong, how often, and where it
started" instead of a wall of repetition.

## 7. Determinism & testing strategy

- **Snapshot tests** (`insta`) pin the exact output of each tool over a fixture
  corpus (`crates/cx-core/tests/fixtures/`). Any output change is a reviewed diff.
- **Property checks** — for code squeezing: *every emitted level must re-parse
  without error* in its source language (a tree-sitter round-trip assertion).
- **Budget invariant** — squeeze output token count ≤ requested budget whenever a
  fitting level exists; otherwise the minimal (L6) level is returned and flagged.
- **Benchmarks** (`criterion`) track reduction ratio and throughput over time.

## 8. Error handling

`cx-core` returns `Result<_, CxError>` (a `thiserror` enum: `Io`, `Parse`,
`UnsupportedLanguage`, `Tokenizer`, …). Wrappers translate `CxError` into their
medium — `cx-mcp` into MCP error payloads, `cx-cli` into `anyhow` reports with
process exit codes. No `unwrap`/`panic` on input-driven paths; `unsafe` is
forbidden workspace-wide.

## 9. Distribution

- A single static `cx-mcp` binary registered with Claude Desktop via stdio.
- A minimal multi-stage Docker image (build on full toolchain, ship the stripped
  binary on a slim base) — the "lightweight local container" from the design.
- Prebuilt cross-platform binaries attached to GitHub Releases (Phase 8).

## 10. Key dependencies

| Crate | Role |
|---|---|
| `tree-sitter` + `tree-sitter-{python,javascript,typescript,go,rust}` | Parsing |
| `tiktoken-rs` | Offline token counting (`cl100k_base`) |
| `ignore` | `.gitignore`-aware directory walking |
| `rmcp` | Official Rust MCP SDK (server, stdio) |
| `clap` | CLI argument parsing |
| `thiserror` / `anyhow` | Library / application error handling |
| `insta` / `criterion` | Snapshot tests / benchmarks |
