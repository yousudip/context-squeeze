# Agent & Contributor Rules — Context Squeeze

Operating rules for any AI agent (Claude Code, etc.) or human working in this
repository. Read this before making changes. It encodes the invariants that keep
the project deterministic, fast, and reviewable.

## What this project is

A **local, deterministic** MCP server that compresses codebases, files, and logs
to fit token budgets using parsing — **never** an LLM. See
[`docs/PROJECT.md`](docs/PROJECT.md) and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Non-negotiable invariants

1. **No LLM / no network on the compression path.** Everything in `cx-core` is
   pure, offline, deterministic. Same input + config ⇒ byte-identical output.
   Do not add embeddings, model calls, or randomness.
2. **No `unsafe`.** It is `forbid`den workspace-wide. Don't add it.
3. **Logic belongs in `cx-core`.** `cx-mcp` and `cx-cli` are thin wrappers
   (parse args / serialize / map errors). No business logic in wrappers.
4. **Squeezing preserves syntax.** Any reduced code must re-parse without error
   in its language. We delete/substitute byte ranges of the *original* source —
   we never pretty-print an AST back to source.
5. **Context preservation > aggression.** Prefer returning slightly more to
   silently dropping a meaningful line. Every elision is explicit/marked.
6. **Honest budgets.** Token counts are an offline `cl100k` approximation; keep
   estimates conservative. Don't claim Claude-exact counts.

## Architecture you must respect

```
cx-mcp ─┐
        ├─► cx-core (tokenizer · ast · skeleton · squeeze · logs · error)
cx-cli ─┘
```

- Add a language: extend the `Language` registry in `ast` + add the grammar
  crate. One match arm, centralized — don't scatter language specifics.
- Reductions are expressed as byte-range edits applied in a single pass.

## Workflow

- Follow the phase plan in [`docs/ROADMAP.md`](docs/ROADMAP.md). When you land a
  tracked item, tick its checkbox in the same change.
- **Definition of done for any code change:**
  ```bash
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
  All three must pass. Output-shaped changes update the matching `insta` snapshot.
- Keep changes scoped; one concern per commit/PR.

## Commit conventions (IMPORTANT)

- **Conventional Commits**: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`,
  `chore:`, `bench:`, `ci:`. Explain *why* in the body when non-obvious.
- **Authorship:** commits are authored by the repository owner. **Do not add
  AI co-author trailers, "Generated with" lines, or any agent attribution** to
  commit messages. Keep the history clean and human-authored in appearance.
- Don't commit secrets; there are none in this project and there should never be.
- Never `push --force` to `main`. Do not skip hooks.

## Testing conventions

- New behavior ships with tests. Prefer testing `cx-core` on in-memory strings
  over filesystem fixtures where possible.
- Snapshot tests (`insta`) pin tool output; review diffs deliberately.
- For squeezing, include the re-parse invariant (output parses cleanly).

## Style

- Idiomatic Rust; `rustfmt` is authoritative. `max_width = 100`.
- Doc-comment public items; `//!` header on each module.
- No `unwrap`/`expect`/`panic` on input-driven paths — return `CxError`.

## When unsure

Prefer the smallest change that satisfies the invariants, leave a focused TODO
referencing the roadmap phase, and surface the open question rather than guessing
at scope. Do not expand into post-`0.1.0` backlog items without being asked.
