//! Throughput benchmarks for the three Context Squeeze operations.
//!
//! Run with `cargo bench -p cx-core`. These track how fast the deterministic
//! engine processes representative inputs over time.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};

use cx_core::tokenizer::Cl100kCounter;
use cx_core::{file_skeleton, squeeze_file, summarize_log_stream, Budget, Language, LogOptions};

const RUST_SOURCE: &str = r#"
//! A representative module with several functions and types.
use std::collections::HashMap;

/// Adds two numbers.
fn add(a: i32, b: i32) -> i32 {
    let total = a + b;
    total
}

/// Counts word frequencies.
fn frequencies(words: &[&str]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for w in words {
        *map.entry(w.to_string()).or_insert(0) += 1;
    }
    map
}

struct Config {
    name: String,
    retries: u32,
    verbose: bool,
}

impl Config {
    fn new(name: String) -> Self {
        Config { name, retries: 3, verbose: false }
    }

    fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
"#;

fn synthetic_log() -> String {
    let mut log = String::new();
    for i in 0..2000 {
        match i % 4 {
            0 => log.push_str(&format!(
                "2026-06-30T10:00:{:02}Z ERROR timeout calling svc-{}\n",
                i % 60,
                i % 7
            )),
            1 => log.push_str(&format!(
                "2026-06-30T10:00:{:02}Z INFO  request served in {}ms\n",
                i % 60,
                i % 50
            )),
            2 => log.push_str(&format!(
                "2026-06-30T10:00:{:02}Z WARN  retry {} for 10.0.0.{}\n",
                i % 60,
                i % 3,
                i % 255
            )),
            _ => log.push_str(&format!(
                "2026-06-30T10:00:{:02}Z DEBUG cache hit 0x{:x}\n",
                i % 60,
                i
            )),
        }
    }
    log
}

fn bench_engine(c: &mut Criterion) {
    let counter = Cl100kCounter::new().unwrap();
    let log = synthetic_log();

    c.bench_function("skeleton_rust_module", |b| {
        b.iter(|| file_skeleton(black_box(RUST_SOURCE), Language::Rust).unwrap())
    });

    c.bench_function("squeeze_rust_module_budget_80", |b| {
        b.iter(|| {
            squeeze_file(
                black_box(RUST_SOURCE),
                Language::Rust,
                Budget::new(80),
                &counter,
            )
            .unwrap()
        })
    });

    c.bench_function("summarize_log_2000_lines", |b| {
        b.iter(|| summarize_log_stream(black_box(&log), &counter, &LogOptions::default()))
    });
}

criterion_group!(benches, bench_engine);
criterion_main!(benches);
