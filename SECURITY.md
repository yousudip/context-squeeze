# Security Policy

## Scope & threat model

Context Squeeze is a **local, offline** tool. It reads files and log text you
point it at and returns compressed text. It makes **no network calls**, requires
**no credentials**, and collects **no telemetry**.

The primary security considerations are therefore:

- **Untrusted input parsing.** The server parses arbitrary source files and log
  text. Inputs must never cause panics, unbounded memory growth, or path escapes.
- **Path handling.** Directory/file tools must not follow paths outside the
  caller-provided root in surprising ways, and must skip device/special files.
- **Resource bounds.** Pathological inputs (huge files, deeply nested ASTs,
  gigantic logs) must degrade gracefully, not exhaust memory.

`unsafe` Rust is `forbid`den across the workspace.

## Supported versions

Pre-`0.1.0`: only the `main` branch is supported. Once releases begin, the latest
minor will receive fixes.

## Reporting a vulnerability

Please **do not** open a public issue for a security problem. Instead use GitHub's
[private vulnerability reporting](https://github.com/yousudip/context-squeeze/security/advisories/new)
for this repository. Include a description, reproduction steps, and impact.

You can expect an acknowledgement within a few days and a coordinated disclosure
once a fix is available.
