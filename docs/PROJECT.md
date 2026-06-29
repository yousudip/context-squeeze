# Context Squeeze — Project Specification

> The authoritative description of *what* Context Squeeze is, *why* it exists,
> and the boundaries of its scope. For *how* it is built, see
> [`ARCHITECTURE.md`](ARCHITECTURE.md); for *when*, see [`ROADMAP.md`](ROADMAP.md).

## 1. One-line definition

Context Squeeze is a local [Model Context Protocol](https://modelcontextprotocol.io)
server that compresses codebases, files, and logs into compact,
structure-preserving projections that fit a caller-supplied token budget —
deterministically, with no LLM in the loop.

## 2. Motivation

Large language models operate inside a finite context window. In practice a
large fraction of what gets loaded into that window is **mechanical noise**:

- comments and docstrings the model can re-derive from code,
- import/boilerplate ceremony,
- blank-line and formatting padding,
- in logs: timestamps, request IDs, and the *same* stack trace repeated hundreds of times.

The common mitigation — a preliminary "summarize this first" LLM pass — has four
problems: it is **slow**, it **costs tokens/money**, it is **non-deterministic**
(the same input yields different summaries), and it is **lossy in unpredictable
ways** (it may drop the one identifier or stack frame needed to solve the task).

Context Squeeze takes the position that **most of this reduction is a parsing
problem, not a reasoning problem**, and should be solved with deterministic
software engineering: tree-sitter for code, line/trace analysis for logs.

## 3. Goals

- **G1 — Deterministic compression.** Identical input + identical configuration
  produces byte-identical output. Fully reproducible, snapshot-testable.
- **G2 — Structure preservation.** Compressed code remains syntactically valid
  and semantically navigable (signatures, types, and call shape survive).
- **G3 — Budget adherence.** Given a token budget, output meets it via a defined,
  predictable degradation ladder — never by arbitrary truncation mid-token.
- **G4 — Context preservation.** Prefer over-inclusion to silently dropping the
  meaningful line. Every elision is explicit and signposted.
- **G5 — Local & private.** No network calls, no API keys, no telemetry. Code
  and logs never leave the host.
- **G6 — Multi-language.** First-class support for Python, TypeScript/JavaScript,
  Go, and Rust, with a registry designed for easy grammar additions.
- **G7 — Showcase-grade engineering.** Idiomatic Rust, thorough tests, CI,
  benchmarks, and documentation worth contributing to.

## 4. Non-goals

- **N1 — Not a summarizer.** It does not paraphrase or "explain" content; it
  *removes* mechanically-redundant material. No semantic rewriting.
- **N2 — No model in the hot path.** No embeddings, no local LLM, no heuristics
  that require inference. (A future *optional* semantic-ranking layer, if ever
  added, would be opt-in and clearly separated.)
- **N3 — Not exact Claude tokenization.** Budgets use an offline approximation
  (see [Architecture §Tokenization](ARCHITECTURE.md#tokenization)); they are
  designed to be safe (slightly conservative), not bit-exact to Anthropic's API.
- **N4 — Not a general code-intelligence server.** No symbol resolution across
  files, no go-to-definition, no type inference beyond what the grammar exposes.

## 5. Users & use cases

| Persona | Use case |
|---|---|
| A developer pairing with Claude on a large repo | `inspect_codebase_skeleton` to give the model a cheap mental map before diving in. |
| Claude debugging a specific module | `fetch_squeezed_file` to load just the relevant file's structure within a tight budget. |
| Anyone triaging a failing CI run | `summarize_log_stream` to turn a giant log into a short error anatomy. |
| Tooling authors | Embed `cx-core` directly as a Rust library for deterministic context reduction. |

## 6. The three capabilities

### 6.1 `inspect_codebase_skeleton(path)`
Walks `path` honoring ignore files, parses each supported source file, and emits
a condensed tree: directories, files, and the **signatures** of top-level and
nested declarations (functions, classes/structs/enums, methods, type aliases) —
bodies omitted. Output is a compact, model-friendly outline of an entire project.

### 6.2 `fetch_squeezed_file(path, token_budget)`
Reads a single file and applies a **degradation ladder** (see Architecture):
strip comments/docstrings → strip blank padding → collapse selected function
bodies to signatures → collapse all bodies → header-only. It stops at the
*first* level that fits `token_budget`, so callers get the richest representation
that still fits. Output is annotated where content was elided.

### 6.3 `summarize_log_stream(raw_text)`
Normalizes log lines (strips timestamps, volatile IDs, ANSI), deduplicates
repeated lines and stack traces (folding them with occurrence counts), groups by
error signature, and returns a compact **error anatomy**: distinct errors, their
frequency, first/last occurrence ordinal, and a representative trace each.

## 7. Success criteria (for `0.1.0` MVP)

- All three tools callable from Claude Desktop over MCP and from the `cx` CLI.
- ≥ 70% median token reduction on the skeleton tool over a representative repo,
  with 100% of emitted code re-parsing without syntax errors.
- Deterministic: golden/snapshot tests pin output for a fixture corpus.
- Reproducible single-binary build; lightweight Docker image.
- Green CI (fmt, clippy `-D warnings`, tests) on Linux/macOS/Windows.

## 8. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Compression drops the line that matters (G4) | Conservative ladder; never collapse below signatures; explicit elision markers; "preserve regions" escape hatch (post-MVP). |
| Token approximation diverges from Claude's real count (N3) | Bias the estimate conservatively; document the gap; keep an optional exact-count adapter seam. |
| Grammar/ABI drift across tree-sitter versions | Pin grammar crate versions; CI matrix; centralize language wiring in one registry module. |
| Multi-language native build friction | Pure-Rust + vendored C grammars compiled by `cc`; verified on all three OSes in CI; reproducible Docker build. |
| Scope creep into "AI summarization" | Non-goals N1/N2 are load-bearing and enforced in review. |

## 9. Glossary

- **Skeleton** — a file/codebase reduced to declaration signatures only.
- **Squeeze** — budget-driven reduction of a single file along the degradation ladder.
- **Degradation ladder** — the ordered sequence of increasingly aggressive
  reductions applied until a token budget is met.
- **Error anatomy** — the structured, deduplicated summary produced from a log stream.
