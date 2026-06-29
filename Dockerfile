# syntax=docker/dockerfile:1

# --- Build stage -------------------------------------------------------------
# A full toolchain (incl. a C compiler for the tree-sitter grammars) compiles a
# stripped, statically-leaning release binary.
FROM rust:1-slim-bookworm AS build
WORKDIR /src

# C toolchain for compiling vendored tree-sitter C grammars.
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first to leverage Docker layer caching for dependencies.
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates/cx-core/Cargo.toml crates/cx-core/Cargo.toml
COPY crates/cx-mcp/Cargo.toml  crates/cx-mcp/Cargo.toml
COPY crates/cx-cli/Cargo.toml  crates/cx-cli/Cargo.toml

# Now the sources and the real build.
COPY crates crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --bin cx-mcp \
    && cp /src/target/release/cx-mcp /usr/local/bin/cx-mcp

# --- Runtime stage -----------------------------------------------------------
# Minimal image that ships only the binary. The server speaks MCP over stdio.
FROM debian:bookworm-slim AS runtime
RUN useradd --create-home --uid 10001 squeeze
COPY --from=build /usr/local/bin/cx-mcp /usr/local/bin/cx-mcp
USER squeeze
ENTRYPOINT ["cx-mcp"]
