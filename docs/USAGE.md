# Using Context Squeeze

Context Squeeze ships an MCP server (`cx-mcp`) that speaks the Model Context
Protocol over stdio, plus a developer CLI (`cx`). This guide covers building it
and registering the server with Claude Desktop.

## Install

**Prebuilt binaries.** Once a release is tagged, download the archive for your
platform from the [Releases page](https://github.com/yousudip/context-squeeze/releases)
and extract `cx` and `cx-mcp`.

**From source:**

```bash
git clone https://github.com/yousudip/context-squeeze
cd context-squeeze
cargo build --release
# binaries: target/release/cx-mcp  and  target/release/cx
```

> Windows needs the MSVC C++ build tools (or VS 2022 "Desktop development with
> C++") so the tree-sitter C grammars can compile.

## Register with Claude Desktop

Claude Desktop launches MCP servers for you and talks to them over stdio. Add an
entry to your `claude_desktop_config.json`:

- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`

```jsonc
{
  "mcpServers": {
    "context-squeeze": {
      "command": "/absolute/path/to/target/release/cx-mcp"
    }
  }
}
```

Restart Claude Desktop. The three tools then appear and Claude can call them.

> The server reads files from the local filesystem with the permissions of the
> launching user. It makes no network calls.

## The tools

### `inspect_codebase_skeleton(path, list_skipped?)`
Returns a signature-only map of a directory: every supported file's functions,
classes, and types as headers, bodies dropped.

### `fetch_squeezed_file(path, token_budget)`
Reads one file and returns the richest representation that fits `token_budget` —
comments/docstrings stripped, padding collapsed, function bodies progressively
collapsed to stubs, falling back to a signature outline for tiny budgets. The
response header reports the level reached and the token reduction.

### `summarize_log_stream(raw_text, max_events?)`
Distills a noisy log into a ranked error anatomy: volatile tokens normalized,
repeated lines and stack traces folded with counts.

## CLI

The `cx` binary mirrors the three tools for local use and scripting; run
`cx --help` once Phase 7 lands. (See [ROADMAP.md](ROADMAP.md).)

## Running via Docker

```bash
docker build -t context-squeeze .
# Mount the code you want it to see and run on stdio:
docker run --rm -i -v "$PWD:/work:ro" context-squeeze
```

To register the containerized server with Claude Desktop, set `"command":
"docker"` with the appropriate `"args"` (`run --rm -i -v ... context-squeeze`).
