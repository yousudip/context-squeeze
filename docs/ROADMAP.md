# Context Squeeze — Roadmap & Development Tracker

This is the **living, fine-grained build plan**. Each phase is a coherent,
shippable unit with a clear Definition of Done (DoD). Checkboxes are updated as
work lands so progress is visible at a glance.

**Legend:** `[ ]` todo · `[~]` in progress · `[x]` done

**Milestone `0.1.0` (MVP)** = Phases 0–8: all three tools working over MCP + CLI,
tested, benchmarked, containerized, documented.

---

## Phase 0 — Foundation & project setup `[x]`

> Goal: a building, linted, documented, CI-backed public repo skeleton.

- [x] Cargo workspace: `cx-core`, `cx-mcp`, `cx-cli`
- [x] Toolchain pin (`rust-toolchain.toml`), `rustfmt.toml`, workspace lints
- [x] MIT `LICENSE`, `.gitignore`
- [x] Docs: `README`, `PROJECT`, `ARCHITECTURE`, `ROADMAP`
- [x] Engine + wrapper stubs compile end-to-end (`cargo check --workspace`)
- [x] Contributor docs: `CONTRIBUTING`, `CODE_OF_CONDUCT`, `SECURITY`
- [x] Agent rules: `CLAUDE.md` / `AGENTS.md`
- [x] CI workflow (fmt + clippy `-D warnings` + test, OS matrix)
- [x] Issue/PR templates
- [x] Dockerfile (multi-stage skeleton)
- [x] `good first issue` / `help wanted` labels
- [x] Public GitHub repo created and pushed

**DoD:** repo is public, CI is green, `cargo check --workspace` passes.

---

## Phase 1 — Tokenizer & Budget Allocator `[x]`

> Goal: deterministic, offline token measurement and a budgeting primitive.

- [x] `error` module: `CxError` (`thiserror`) + `Result` alias
- [x] `TokenCounter` trait + `Cl100kCounter` impl over `tiktoken-rs`
- [x] `TokenCount` newtype + `Budget` type (target, headroom, conservative bias)
- [x] `Budget::fits(&self, count)` and remaining-headroom helpers
- [x] Unit tests: known strings → expected counts; budget arithmetic
- [ ] Bench: counting throughput (`criterion`) — _deferred to Phase 7 with the other benches_

**DoD:** counting is stable across runs; budget logic unit-tested. ✅

---

## Phase 2 — AST Engine & language registry `[x]`

> Goal: parse all target languages and classify declaration/body nodes.

- [x] `Language` enum (Python, JS, TS, TSX, Go, Rust) + extension detection
- [x] Grammar wiring (`ts_language`) verified loadable for every language
- [x] Per-language node-kind tables (declaration kinds, body/name field, comments)
- [x] `parse(source, lang) -> Tree` + safe error surfacing (`CxError::Parse`)
- [x] `declarations(...)` walker yielding `(kind, name, depth, header, body)` spans
- [x] Tests per language assert declarations found (incl. Go name fallback)
- [x] Round-trip invariant helper (`parses_cleanly`)

**DoD:** every target language parses; declarations enumerated with spans. ✅

---

## Phase 3 — Skeleton extraction (`inspect_codebase_skeleton`) `[x]`

> Goal: condensed, signature-only map of a directory tree.

- [x] `ignore`-based walker (respect `.gitignore`/hidden, size cap, sorted output)
- [x] Per-file skeletonizer: keep declaration headers, drop bodies
- [x] Nested declarations (methods within classes/impls) preserved with indentation
- [x] Compact per-file output format + token accounting (`SkeletonReport`)
- [x] Graceful handling of unsupported/too-large/unreadable files (listed, not parsed)
- [x] Snapshot test over a mixed-language fixture project (Rust/Python/TS/Go)
- [x] Reduction-ratio assertion (> 45% on fixtures)

**DoD:** real directory → faithful skeleton; snapshot-pinned; ratio measured. ✅

> Note: type bodies (struct fields, enum variants) are dropped in the
> signature-only view; surfacing them is tracked in the post-`0.1.0` backlog.

---

## Phase 4 — Semantic file squeezing (`fetch_squeezed_file`) `[x]`

> Goal: budget-driven single-file compression along the degradation ladder.

- [x] L1 strip comments + safe Python docstrings (AST node kinds)
- [x] L2 collapse blank/padding runs (`normalize_ws`)
- [x] L3 greedy body collapse by body size (largest first)
- [x] L4 collapse all bodies; L5 skeleton; L6 truncated-with-marker
- [x] Budget-fitting loop: first level that fits wins
- [x] Elision metadata (`SqueezeResult`: level, before/after tokens, bodies collapsed, validity)
- [x] Invariant test: every valid-source level (L0–L4) re-parses without error
- [x] Budget invariant: `fits_budget` reflects effective limit; cross-budget tests
- [x] Byte-range edit engine (`apply_edits`) with overlap-safe collapsing

**DoD:** budgets honored; syntax always valid (L0–L4); elisions reported. ✅

---

## Phase 5 — Log stream summarization (`summarize_log_stream`) `[x]`

> Goal: collapse a large log into a compact error anatomy.

- [x] Line normalization (ANSI, timestamps, UUID/hex/IP/long-ints → canonical)
- [x] Stack-trace folding (indented frames + Python traceback terminator state)
- [x] Clustering by signature (incl. deepest frame) with counts + first/last line
- [x] Severity ranking (fatal > error > warn > info > debug) then frequency
- [x] Error-anatomy output format + token accounting (`LogSummary`)
- [x] Tests: dedup counts, ranking, traceback distinction, >90% reduction

**DoD:** big repetitive logs reduce to a stable, ranked error map. ✅

> Note: Go/JVM "Caused by" multi-segment traces fold partially; deeper
> language-specific trace grammars are tracked in the backlog.

---

## Phase 6 — MCP server wiring (`cx-mcp`) `[x]`

> Goal: expose the three tools to Claude Desktop over stdio.

- [x] Pinned `rmcp` 2.0 (`server`, `macros`, `transport-io`, `schemars`)
- [x] `ServerHandler` + `#[tool_router]`/`#[tool_handler]` with three typed tools
- [x] `Parameters` structs with `schemars` JSON schemas + descriptions
- [x] `CxError` → MCP error mapping (`internal_error`/`invalid_params`)
- [x] stdio transport + EOF-driven shutdown; server identity set to `context-squeeze`
- [x] Smoke test: scripted `initialize` → `tools/list` → `tools/call` over stdio ✔
- [x] `docs/USAGE.md`: build + Claude Desktop config snippet

**DoD:** Claude Desktop can call all three tools and get correct output. ✅

---

## Phase 7 — CLI, golden tests & benchmarks `[x]`

> Goal: first-class local UX and a regression-proof test/bench suite.

- [x] `clap` CLI: `skeleton`, `squeeze`, `logs` subcommands + flags
- [x] Human-readable (stats to stderr, result to stdout) and `--json` output modes
- [x] `logs` reads from a file or stdin
- [x] Golden tests driving the built `cx` binary end-to-end (6 tests)
- [x] `criterion` bench (`engine`) for skeleton/squeeze/log throughput
- [x] Sample corpus under `examples/` (Python file + noisy log) for demos

**DoD:** `cx` is usable standalone; golden tests run in CI; benches available. ✅

---

## Phase 8 — Packaging, Docker & releases `[ ]`

> Goal: easy install; reproducible, lightweight distribution.

- [ ] Multi-stage Dockerfile (slim runtime, stripped static binary)
- [ ] Release workflow: cross-compile Linux/macOS/Windows, attach to Releases
- [ ] `cargo install` + Docker usage docs; MCP config recipes
- [ ] Optional: publish `cx-core` to crates.io

**DoD:** a user can install and register the server in minutes.

---

## Post-`0.1.0` — Backlog / stretch

- [ ] "Preserve regions": pragmas/markers to pin spans against squeezing
- [ ] More grammars (Java, C/C++, Ruby, C#, PHP) via the registry
- [ ] Optional exact token counts via Anthropic `count_tokens` adapter
- [ ] Incremental re-skeletonization on file change (watch mode)
- [ ] Diff-aware squeezing (squeeze a git diff's context)
- [ ] Configurable ladder/policy via a `cx.toml`
- [ ] Optional semantic ranking layer (clearly opt-in; off the deterministic path)

---

### Changelog of plan changes
Material changes to scope or sequence are recorded here so the history of the
plan stays auditable.

- `2026-06-29` — Initial roadmap drafted; Phase 0 scaffolding underway.
