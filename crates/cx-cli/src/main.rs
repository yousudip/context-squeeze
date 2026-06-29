//! Context Squeeze — developer CLI (`cx`).
//!
//! Mirrors the three MCP tools as terminal subcommands so the compression engine
//! can be exercised, scripted, golden-tested, and benchmarked without an MCP
//! client. Like the MCP server, this is a thin wrapper over `cx-core`.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;

use cx_core::{
    codebase_skeleton, squeeze_file, summarize_log_stream, Budget, Cl100kCounter, Language,
    LogOptions, SkeletonOptions,
};

/// Deterministic, local context compression for codebases, files, and logs.
#[derive(Debug, Parser)]
#[command(name = "cx", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print a signature-only skeleton of a directory or file.
    Skeleton {
        /// Path to the directory (or file) to map.
        path: PathBuf,
        /// Do not list files that were skipped.
        #[arg(long)]
        no_skipped: bool,
        /// Emit a JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Squeeze a single source file to fit a token budget.
    Squeeze {
        /// Path to the source file.
        path: PathBuf,
        /// Target token budget.
        #[arg(short, long)]
        budget: usize,
        /// Emit a JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Summarize a log stream (from a file or stdin) into an error anatomy.
    Logs {
        /// Path to the log file. Reads from stdin when omitted.
        path: Option<PathBuf>,
        /// Maximum number of distinct events to show.
        #[arg(long, default_value_t = 40)]
        max_events: usize,
        /// Emit a JSON object instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let counter = Cl100kCounter::new().map_err(|e| anyhow::anyhow!(e.to_string()))?;

    match cli.command {
        Command::Skeleton {
            path,
            no_skipped,
            json,
        } => {
            let opts = SkeletonOptions {
                list_skipped: !no_skipped,
                ..Default::default()
            };
            let report = codebase_skeleton(&path, &counter, &opts)
                .with_context(|| format!("skeletonizing {}", path.display()))?;
            if json {
                print_json(&json!({
                    "path": path.display().to_string(),
                    "files": report.parsed_count(),
                    "original_tokens": report.original_tokens.get(),
                    "skeleton_tokens": report.skeleton_tokens.get(),
                    "reduction": report.reduction_ratio(),
                    "skeleton": report.rendered,
                }));
            } else {
                eprintln!(
                    "{} file(s), {} → {} tokens ({:.0}% reduction)",
                    report.parsed_count(),
                    report.original_tokens,
                    report.skeleton_tokens,
                    report.reduction_ratio() * 100.0,
                );
                print!("{}", report.rendered);
            }
        }

        Command::Squeeze { path, budget, json } => {
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let language = Language::from_path(&path)
                .with_context(|| format!("unsupported file type: {}", path.display()))?;
            let result = squeeze_file(&source, language, Budget::new(budget), &counter)
                .with_context(|| format!("squeezing {}", path.display()))?;
            if json {
                print_json(&json!({
                    "path": path.display().to_string(),
                    "language": language.name(),
                    "level": result.level.to_string(),
                    "is_valid_source": result.is_valid_source,
                    "original_tokens": result.original_tokens.get(),
                    "output_tokens": result.output_tokens.get(),
                    "reduction": result.reduction_ratio(),
                    "bodies_collapsed": result.bodies_collapsed,
                    "fits_budget": result.fits_budget,
                    "output": result.output,
                }));
            } else {
                eprintln!(
                    "{} [{}] — {} ({} → {} tokens, {:.0}% reduction){}",
                    path.display(),
                    language.name(),
                    result.level,
                    result.original_tokens,
                    result.output_tokens,
                    result.reduction_ratio() * 100.0,
                    if result.is_valid_source {
                        ""
                    } else {
                        " [outline]"
                    },
                );
                print!("{}", result.output);
            }
        }

        Command::Logs {
            path,
            max_events,
            json,
        } => {
            let raw = match &path {
                Some(p) => std::fs::read_to_string(p)
                    .with_context(|| format!("reading {}", p.display()))?,
                None => read_stdin().context("reading stdin")?,
            };
            let opts = LogOptions {
                max_events,
                ..Default::default()
            };
            let summary = summarize_log_stream(&raw, &counter, &opts);
            if json {
                print_json(&json!({
                    "input_lines": summary.input_lines,
                    "total_records": summary.total_records,
                    "distinct_events": summary.events.len(),
                    "original_tokens": summary.original_tokens.get(),
                    "summary_tokens": summary.summary_tokens.get(),
                    "reduction": summary.reduction_ratio(),
                    "summary": summary.rendered,
                }));
            } else {
                eprintln!(
                    "{} lines → {} event(s) ({:.0}% reduction)",
                    summary.input_lines,
                    summary.events.len(),
                    summary.reduction_ratio() * 100.0,
                );
                print!("{}", summary.rendered);
            }
        }
    }

    Ok(())
}

fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serialize JSON")
    );
}

fn read_stdin() -> std::io::Result<String> {
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}
