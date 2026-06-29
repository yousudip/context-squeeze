<!-- Thanks for contributing to Context Squeeze! -->

## What & why

<!-- What does this change do, and why? Link any issue: Closes #123 -->

## Roadmap

<!-- Which docs/ROADMAP.md phase/item does this advance? Tick its checkbox there. -->

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo test --workspace` passes
- [ ] Snapshots updated if output changed (`cargo insta review`)
- [ ] No business logic added to `cx-mcp` / `cx-cli` (logic lives in `cx-core`)
- [ ] No new `unsafe`, no LLM/network on the compression path
