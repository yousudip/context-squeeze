//! Log-stream summarization — the engine behind `summarize_log_stream`.
//!
//! Turns a large, repetitive log into a compact **error anatomy**: lines are
//! normalized (ANSI, timestamps, and high-cardinality tokens canonicalized),
//! multi-line stack traces are folded into their preceding record, identical
//! records are clustered with occurrence counts, and the result is ranked by
//! severity then frequency.

use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;

use regex::Regex;

use crate::tokenizer::{TokenCount, TokenCounter};

/// Severity level detected for a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Other = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl Level {
    fn detect(line: &str) -> Level {
        let upper = line.to_ascii_uppercase();
        let has = |needle: &str| upper.contains(needle);
        if has("FATAL") || has("CRITICAL") {
            Level::Fatal
        } else if has("ERROR")
            || has("PANIC")
            || has("TRACEBACK")
            || has("EXCEPTION")
            || has(" ERR ")
        {
            Level::Error
        } else if has("WARN") {
            Level::Warn
        } else if has("INFO") {
            Level::Info
        } else if has("DEBUG") || has("TRACE") {
            Level::Debug
        } else {
            Level::Other
        }
    }

    /// The level's tag as shown in the rendered anatomy.
    pub fn tag(self) -> &'static str {
        match self {
            Level::Fatal => "FATAL",
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Other => "LOG",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.tag())
    }
}

/// Options controlling log summarization.
#[derive(Debug, Clone)]
pub struct LogOptions {
    /// Maximum number of distinct events to render (the rest are summarized as
    /// an elided count).
    pub max_events: usize,
    /// Maximum number of lines shown for each event's representative record.
    pub max_record_lines: usize,
    /// Only render events at or above this severity.
    pub min_level: Level,
}

impl Default for LogOptions {
    fn default() -> Self {
        Self {
            max_events: 40,
            max_record_lines: 16,
            min_level: Level::Other,
        }
    }
}

/// A single clustered event in the summary.
#[derive(Debug, Clone)]
pub struct LogEvent {
    /// Severity of the event.
    pub level: Level,
    /// How many records collapsed into this event.
    pub count: usize,
    /// 1-based input line where this event was first seen.
    pub first_line: usize,
    /// 1-based input line where this event was last seen.
    pub last_line: usize,
    /// The original text of the first occurrence (primary line + folded trace).
    pub representative: String,
}

/// The result of summarizing a log stream.
#[derive(Debug, Clone)]
pub struct LogSummary {
    /// Distinct clustered events, ranked by severity then frequency.
    pub events: Vec<LogEvent>,
    /// The model-ready rendered anatomy.
    pub rendered: String,
    /// Number of non-blank input lines.
    pub input_lines: usize,
    /// Number of records (primary lines + folded traces) before clustering.
    pub total_records: usize,
    /// Tokens in the original raw log.
    pub original_tokens: TokenCount,
    /// Tokens in the rendered summary.
    pub summary_tokens: TokenCount,
}

impl LogSummary {
    /// Fraction of tokens removed vs. the original log, in `[0, 1]`.
    pub fn reduction_ratio(&self) -> f32 {
        let original = self.original_tokens.get();
        if original == 0 {
            return 0.0;
        }
        let kept = self.summary_tokens.get().min(original);
        1.0 - (kept as f32 / original as f32)
    }
}

struct Patterns {
    ansi: Regex,
    iso_ts: Regex,
    time_ts: Regex,
    uuid: Regex,
    hex_addr: Regex,
    ipv4: Regex,
    long_num: Regex,
    ws: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        ansi: Regex::new(r"\x1b\[[0-9;]*m").unwrap(),
        iso_ts: Regex::new(
            r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?",
        )
        .unwrap(),
        time_ts: Regex::new(r"\d{2}:\d{2}:\d{2}(?:\.\d+)?").unwrap(),
        uuid: Regex::new(
            r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
        )
        .unwrap(),
        hex_addr: Regex::new(r"0x[0-9a-fA-F]+").unwrap(),
        ipv4: Regex::new(r"\b\d{1,3}(?:\.\d{1,3}){3}\b").unwrap(),
        long_num: Regex::new(r"\b\d{4,}\b").unwrap(),
        ws: Regex::new(r"\s+").unwrap(),
    })
}

/// Canonicalize a line so that records differing only in volatile tokens
/// (timestamps, ids, addresses) cluster together.
fn normalize(line: &str) -> String {
    let p = patterns();
    let s = p.ansi.replace_all(line, "");
    let s = p.iso_ts.replace_all(&s, "<TS>");
    let s = p.time_ts.replace_all(&s, "<TS>");
    let s = p.uuid.replace_all(&s, "<UUID>");
    let s = p.hex_addr.replace_all(&s, "<HEX>");
    let s = p.ipv4.replace_all(&s, "<IP>");
    let s = p.long_num.replace_all(&s, "<N>");
    p.ws.replace_all(&s, " ").trim().to_string()
}

/// Whether `line` continues the preceding record (a stack-trace frame or wrapped
/// line) rather than starting a new one.
fn is_continuation(line: &str) -> bool {
    if line.starts_with(' ') || line.starts_with('\t') {
        return true;
    }
    let t = line.trim_start();
    t.starts_with("at ")
        || t.starts_with("Caused by:")
        || t.starts_with("... ")
        || t.starts_with("File \"")
}

struct Cluster {
    level: Level,
    count: usize,
    first_line: usize,
    last_line: usize,
    representative: String,
}

/// Summarize `raw_text` into a clustered error anatomy.
pub fn summarize_log_stream(
    raw_text: &str,
    counter: &impl TokenCounter,
    opts: &LogOptions,
) -> LogSummary {
    let original_tokens = counter.count(raw_text);

    // 1. Group lines into records (primary line + folded continuations).
    struct Record {
        signature: String,
        level: Level,
        first_line: usize,
        original_lines: Vec<String>,
    }
    let mut records: Vec<Record> = Vec::new();
    let mut input_lines = 0usize;
    // A Python traceback's terminating exception line ("ValueError: …") is not
    // indented, so once a traceback is open we keep folding lines until that
    // non-indented terminator closes it.
    let mut open_traceback = false;

    for (idx, line) in raw_text.lines().enumerate() {
        if line.trim().is_empty() {
            open_traceback = false;
            continue;
        }
        input_lines += 1;
        let cont = is_continuation(line);

        if open_traceback {
            if let Some(rec) = records.last_mut() {
                rec.original_lines.push(line.to_string());
                if !cont {
                    open_traceback = false; // exception terminator closes the trace
                }
                continue;
            }
            open_traceback = false;
        }

        if cont {
            if let Some(rec) = records.last_mut() {
                rec.original_lines.push(line.to_string());
                continue;
            }
        }

        records.push(Record {
            signature: normalize(line),
            level: Level::detect(line),
            first_line: idx + 1,
            original_lines: vec![line.to_string()],
        });
        if line
            .trim_start()
            .starts_with("Traceback (most recent call last):")
        {
            open_traceback = true;
        }
    }

    // The signature folds in the last frame so different errors sharing a generic
    // primary line (e.g. "Traceback (most recent call last):") stay distinct.
    for rec in &mut records {
        if rec.original_lines.len() > 1 {
            let last = rec.original_lines.last().unwrap();
            rec.signature = format!("{}\u{b6}{}", rec.signature, normalize(last));
            // The deepest frame usually carries the real error level.
            rec.level = rec.level.max(Level::detect(last));
        }
    }

    let total_records = records.len();

    // 2. Cluster by signature.
    let mut clusters: HashMap<String, Cluster> = HashMap::new();
    for rec in records {
        let entry = clusters
            .entry(rec.signature.clone())
            .or_insert_with(|| Cluster {
                level: rec.level,
                count: 0,
                first_line: rec.first_line,
                last_line: rec.first_line,
                representative: rec.original_lines.join("\n"),
            });
        entry.count += 1;
        entry.level = entry.level.max(rec.level);
        entry.first_line = entry.first_line.min(rec.first_line);
        entry.last_line = entry.last_line.max(rec.first_line);
    }

    // 3. Rank: severity desc, then frequency desc, then first occurrence.
    let mut events: Vec<LogEvent> = clusters
        .into_values()
        .map(|c| LogEvent {
            level: c.level,
            count: c.count,
            first_line: c.first_line,
            last_line: c.last_line,
            representative: c.representative,
        })
        .filter(|e| e.level >= opts.min_level)
        .collect();
    events.sort_by(|a, b| {
        b.level
            .cmp(&a.level)
            .then(b.count.cmp(&a.count))
            .then(a.first_line.cmp(&b.first_line))
    });

    let rendered = render(&events, total_records, input_lines, opts);
    let summary_tokens = counter.count(&rendered);

    LogSummary {
        events,
        rendered,
        input_lines,
        total_records,
        original_tokens,
        summary_tokens,
    }
}

fn render(
    events: &[LogEvent],
    total_records: usize,
    input_lines: usize,
    opts: &LogOptions,
) -> String {
    let shown = events.len().min(opts.max_events);
    let mut out = String::new();
    out.push_str(&format!(
        "# Log summary: {input_lines} lines, {total_records} records \u{2192} {} distinct event(s)\n\n",
        events.len()
    ));

    for event in events.iter().take(shown) {
        let location = if event.first_line == event.last_line {
            format!("line {}", event.first_line)
        } else {
            format!("lines {}\u{2013}{}", event.first_line, event.last_line)
        };
        out.push_str(&format!(
            "## [{} \u{d7}{}] {}\n",
            event.level.tag(),
            event.count,
            location
        ));

        let lines: Vec<&str> = event.representative.lines().collect();
        for line in lines.iter().take(opts.max_record_lines) {
            out.push_str(line);
            out.push('\n');
        }
        if lines.len() > opts.max_record_lines {
            out.push_str(&format!(
                "  \u{2026} [{} more line(s)]\n",
                lines.len() - opts.max_record_lines
            ));
        }
        out.push('\n');
    }

    if events.len() > shown {
        out.push_str(&format!(
            "\u{2026} [{} more event(s) elided]\n",
            events.len() - shown
        ));
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::Cl100kCounter;

    fn counter() -> Cl100kCounter {
        Cl100kCounter::new().unwrap()
    }

    #[test]
    fn repeated_lines_collapse_with_counts() {
        let log = "\
2026-06-30T10:00:01Z ERROR db connection to 10.0.0.1 failed
2026-06-30T10:00:02Z ERROR db connection to 10.0.0.2 failed
2026-06-30T10:00:03Z ERROR db connection to 10.0.0.3 failed
2026-06-30T10:00:04Z INFO  request served in 12ms
";
        let summary = summarize_log_stream(log, &counter(), &LogOptions::default());
        // The three connection errors normalize to one event with count 3.
        let db = summary
            .events
            .iter()
            .find(|e| e.representative.contains("db connection"))
            .unwrap();
        assert_eq!(db.count, 3);
        assert_eq!(db.level, Level::Error);
        // Two distinct events total (error + info).
        assert_eq!(summary.events.len(), 2);
    }

    #[test]
    fn errors_rank_above_info() {
        let log = "\
INFO starting up
INFO ready
INFO ready
WARN disk space low
ERROR unhandled exception in handler
";
        let summary = summarize_log_stream(log, &counter(), &LogOptions::default());
        assert_eq!(summary.events[0].level, Level::Error);
        assert!(
            summary.rendered.contains("[ERROR \u{d7}1]"),
            "{}",
            summary.rendered
        );
    }

    #[test]
    fn python_tracebacks_fold_and_distinguish() {
        let log = "\
Traceback (most recent call last):
  File \"app.py\", line 10, in main
    do_thing()
ValueError: bad value
Traceback (most recent call last):
  File \"app.py\", line 42, in other
    do_other()
KeyError: 'missing'
";
        let summary = summarize_log_stream(log, &counter(), &LogOptions::default());
        // Two distinct tracebacks despite the identical primary line.
        assert_eq!(summary.events.len(), 2, "{}", summary.rendered);
        assert!(
            summary.rendered.contains("ValueError: bad value"),
            "{}",
            summary.rendered
        );
        assert!(
            summary.rendered.contains("KeyError"),
            "{}",
            summary.rendered
        );
    }

    #[test]
    fn large_repetitive_log_reduces_a_lot() {
        let mut log = String::new();
        for i in 0..500 {
            log.push_str(&format!(
                "2026-06-30T10:00:{:02}Z ERROR timeout calling service\n",
                i % 60
            ));
        }
        let summary = summarize_log_stream(&log, &counter(), &LogOptions::default());
        assert_eq!(summary.events.len(), 1);
        assert_eq!(summary.events[0].count, 500);
        assert!(
            summary.reduction_ratio() > 0.9,
            "ratio {}",
            summary.reduction_ratio()
        );
    }

    #[test]
    fn empty_input_is_handled() {
        let summary = summarize_log_stream("", &counter(), &LogOptions::default());
        assert_eq!(summary.events.len(), 0);
        assert_eq!(summary.total_records, 0);
    }

    #[test]
    fn normalize_canonicalizes_volatile_tokens() {
        assert_eq!(
            normalize("2026-06-30T10:00:01Z req 0xDEADBEEF from 192.168.1.5 id 1234567"),
            "<TS> req <HEX> from <IP> id <N>"
        );
    }
}
