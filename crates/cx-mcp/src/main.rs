//! Context Squeeze — MCP (Model Context Protocol) stdio server.
//!
//! Exposes the three Context Squeeze tools to Claude Desktop and any other MCP
//! client over stdio. All compression logic lives in `cx-core`; this binary is
//! a thin transport wrapper that maps MCP tool calls onto the engine.

use std::path::Path;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{
    schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use serde::Deserialize;

use cx_core::{
    codebase_skeleton, squeeze_file, summarize_log_stream, Budget, Cl100kCounter, Language,
    LogOptions, SkeletonOptions,
};

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SkeletonRequest {
    #[schemars(
        description = "Path to the directory (or file) to map. Relative paths resolve against the server's working directory."
    )]
    path: String,
    #[schemars(
        description = "Whether to list files that were skipped (unsupported type, too large). Defaults to true."
    )]
    #[serde(default)]
    list_skipped: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SqueezeRequest {
    #[schemars(description = "Path to the source file to read and compress.")]
    path: String,
    #[schemars(
        description = "Target token budget. The richest representation that fits within this budget is returned."
    )]
    token_budget: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LogRequest {
    #[schemars(description = "The raw log text to summarize.")]
    raw_text: String,
    #[schemars(description = "Maximum number of distinct events to include. Defaults to 40.")]
    #[serde(default)]
    max_events: Option<u32>,
}

/// The MCP server: holds the (shared) token counter and the generated tool router.
#[derive(Clone)]
struct SqueezeServer {
    counter: Arc<Cl100kCounter>,
    // Dispatched through by the `#[tool_handler]`-generated code; the dead-code
    // analyzer doesn't see that use.
    #[allow(dead_code)]
    tool_router: ToolRouter<SqueezeServer>,
}

#[tool_router]
impl SqueezeServer {
    fn new() -> anyhow::Result<Self> {
        let counter = Cl100kCounter::new().map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(Self {
            counter: Arc::new(counter),
            tool_router: Self::tool_router(),
        })
    }

    #[tool(
        description = "Map a codebase to a compact, signature-only skeleton: each supported source file's functions, classes, and types are listed as headers with their bodies dropped. Use this to cheaply understand the shape of a directory before diving into specific files. Supports Python, JavaScript, TypeScript, Go, and Rust."
    )]
    async fn inspect_codebase_skeleton(
        &self,
        Parameters(req): Parameters<SkeletonRequest>,
    ) -> Result<CallToolResult, McpError> {
        let opts = SkeletonOptions {
            list_skipped: req.list_skipped.unwrap_or(true),
            ..Default::default()
        };
        let report = codebase_skeleton(Path::new(&req.path), self.counter.as_ref(), &opts)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let header = format!(
            "# Skeleton of `{}` — {} file(s), {} → {} tokens ({:.0}% reduction)\n\n",
            req.path,
            report.parsed_count(),
            report.original_tokens,
            report.skeleton_tokens,
            report.reduction_ratio() * 100.0,
        );
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "{header}{}",
            report.rendered
        ))]))
    }

    #[tool(
        description = "Read a single source file and compress it to fit a token budget without breaking syntax. Strips comments and docstrings, collapses padding, then progressively collapses function bodies to stubs — returning the richest representation that fits. Falls back to a signature-only outline for very small budgets."
    )]
    async fn fetch_squeezed_file(
        &self,
        Parameters(req): Parameters<SqueezeRequest>,
    ) -> Result<CallToolResult, McpError> {
        let source = std::fs::read_to_string(&req.path)
            .map_err(|e| McpError::internal_error(format!("reading {}: {e}", req.path), None))?;
        let language = Language::from_path(Path::new(&req.path)).ok_or_else(|| {
            McpError::invalid_params(format!("unsupported file type: {}", req.path), None)
        })?;
        let budget = Budget::new(req.token_budget as usize);
        let result = squeeze_file(&source, language, budget, self.counter.as_ref())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let validity = if result.is_valid_source {
            ""
        } else {
            " [outline — not valid source]"
        };
        let header = format!(
            "# `{}` [{}] — {} ({} → {} tokens, {:.0}% reduction){}\n\n",
            req.path,
            language.name(),
            result.level,
            result.original_tokens,
            result.output_tokens,
            result.reduction_ratio() * 100.0,
            validity,
        );
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "{header}{}",
            result.output
        ))]))
    }

    #[tool(
        description = "Distill a large, noisy log into a compact error anatomy: timestamps and volatile tokens are normalized, repeated lines and stack traces are folded with occurrence counts, and the result is ranked by severity then frequency. Turns tens of thousands of log tokens into a short, deduplicated summary."
    )]
    async fn summarize_log_stream(
        &self,
        Parameters(req): Parameters<LogRequest>,
    ) -> Result<CallToolResult, McpError> {
        let opts = LogOptions {
            max_events: req
                .max_events
                .map(|n| n as usize)
                .unwrap_or(LogOptions::default().max_events),
            ..Default::default()
        };
        let summary = summarize_log_stream(&req.raw_text, self.counter.as_ref(), &opts);
        Ok(CallToolResult::success(vec![ContentBlock::text(
            summary.rendered,
        )]))
    }
}

#[tool_handler]
impl ServerHandler for SqueezeServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Context Squeeze: deterministic, local context compression. \
                 inspect_codebase_skeleton(path) returns a signature-only map of a directory; \
                 fetch_squeezed_file(path, token_budget) loads one file compressed to fit a budget; \
                 summarize_log_stream(raw_text) distills a noisy log into a ranked error anatomy. \
                 All processing is local and deterministic — no data leaves the machine."
                    .to_string(),
            );
        info.server_info.name = "context-squeeze".to_string();
        info.server_info.version = cx_core::VERSION.to_string();
        info
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // MCP speaks JSON-RPC over stdout, so all diagnostics must go to stderr.
    eprintln!("cx-mcp {} starting on stdio", cx_core::VERSION);

    let server = SqueezeServer::new()?;
    let service = server.serve(stdio()).await.inspect_err(|e| {
        eprintln!("cx-mcp failed to start: {e}");
    })?;
    service.waiting().await?;
    Ok(())
}
