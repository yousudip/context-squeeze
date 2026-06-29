# CLAUDE.md

Guidance for Claude Code (and other agents) working in this repository.

**👉 The authoritative rules live in [`AGENTS.md`](AGENTS.md). Read it first and
follow it.** This file exists so Claude Code picks up the same rules.

## TL;DR of the invariants (full detail in AGENTS.md)

- **Deterministic, offline, no LLM/network on the compression path.** Same input ⇒ same output.
- **No `unsafe`** (forbidden workspace-wide).
- **All logic in `cx-core`**; `cx-mcp` and `cx-cli` are thin wrappers.
- **Squeezed code must re-parse** — edit byte ranges of original source, never re-print an AST.
- **Preserve context over aggressive trimming**; mark every elision.
- **Budgets are a conservative offline `cl100k` approximation**, not Claude-exact.

## Definition of done for code changes

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Commits

Conventional Commits. **Do not add AI co-authorship, "Generated with", or any
agent attribution trailers** — history is authored by the repository owner.

## Orientation

- Plan & progress: [`docs/ROADMAP.md`](docs/ROADMAP.md)
- What/why: [`docs/PROJECT.md`](docs/PROJECT.md)
- How: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
