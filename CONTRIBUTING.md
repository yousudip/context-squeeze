# Contributing to Context Squeeze

Thanks for your interest — contributions are genuinely welcome. This project aims
to be a clean, well-tested, deterministic piece of systems software, and that
only happens with care from everyone who touches it.

## Ways to contribute

- **Parser edge cases** — a construct that skeletonizes or squeezes wrong is a
  great, well-scoped bug to fix (attach the snippet).
- **New language grammars** — the `Language` registry is built to make this a
  small, self-contained change. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md#3-the-ast-engine).
- **Budgeting & log heuristics** — better degradation/clustering with tests.
- **Docs, examples, benchmarks** — always appreciated.

Look for [`good first issue`](https://github.com/yousudip/context-squeeze/labels/good%20first%20issue)
and [`help wanted`](https://github.com/yousudip/context-squeeze/labels/help%20wanted).

## Ground rules

1. **Determinism is sacred.** No nondeterministic ordering, timestamps, or
   model calls on the compression path. Same input ⇒ same output.
2. **Logic lives in `cx-core`.** `cx-mcp` and `cx-cli` are thin wrappers; don't
   put business logic in them.
3. **No `unsafe`.** It is `forbid`den workspace-wide.
4. **Tests with behavior.** New behavior ships with tests; output-shaped changes
   update the relevant snapshot in the same PR.

## Development setup

```bash
# Rust 1.82+ via rustup (the repo pins a toolchain)
git clone https://github.com/yousudip/context-squeeze
cd context-squeeze

cargo build --workspace
cargo test  --workspace
```

> On Windows the MSVC C++ build tools (or VS 2022 "Desktop development with C++")
> are required so the tree-sitter C grammars can compile.

## Before you open a PR

Run the same gate CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Reviewing snapshot changes: `cargo insta review` (install `cargo-insta`).

## Commit & PR conventions

- **Conventional Commits** (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
  `chore:`, `bench:`). Keep commits focused and message bodies explaining *why*.
- Reference the roadmap phase where relevant (e.g. "Phase 4").
- Keep PRs scoped to one concern; update [`docs/ROADMAP.md`](docs/ROADMAP.md)
  checkboxes when a tracked item lands.
- By contributing you agree your work is licensed under the project's
  [MIT License](LICENSE).

## Code style

- Idiomatic Rust; `rustfmt` is the source of truth for formatting.
- Public items get doc comments; modules carry a `//!` summary.
- Prefer total, panic-free functions on input-driven paths; return `CxError`.

## Conduct

Participation is governed by our [Code of Conduct](CODE_OF_CONDUCT.md).
