//! Replace All micro-benchmark: compares legacy strategies vs the optimized path.
//!
//! Run: `cargo run --release -p editor-text-engine --example replace_all_bench`

use std::time::{Duration, Instant};

use editor_text_engine::document::Document;
use editor_text_engine::history::{Edit, Transaction};
use editor_text_engine::search::{SearchOptions, Searcher};
use editor_text_engine::selection::SelectionSet;
use editor_text_engine::TextBuffer;
use regex::Regex;

const WARMUP: u32 = 2;
const ITERS: u32 = 5;

fn main() {
    println!("Replace All benchmark (release build recommended)\n");
    println!(
        "{:<28} {:>10} {:>10} {:>10} {:>8}",
        "Scenario", "String", "N-edits", "Optimized", "Speedup"
    );
    println!("{}", "-".repeat(72));

    bench_dense_matches();
    bench_many_lines();
    bench_large_sparse();
}

fn bench_dense_matches() {
    // ~1 MiB, "foo" every 40 bytes → ~26k matches
    let text = make_dense("foo", 40, 1 << 20);
    run_row("1 MiB dense (foo→bar)", &text, "foo", "bar");
}

fn bench_many_lines() {
    // ~1 MiB, one "needle" per 80-char line → ~13k lines
    let line = format!("// comment needle padding xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n");
    let reps = (1 << 20) / line.len();
    let text: String = line.repeat(reps);
    run_row("1 MiB one/line", &text, "needle", "MATCH");
}

fn bench_large_sparse() {
    // ~10 MiB, match every 4 KiB → ~2.5k matches
    let text = make_dense("MARK", 4096, 10 << 20);
    run_row("10 MiB sparse (MARK→X)", &text, "MARK", "X");
}

fn make_dense(needle: &str, spacing: usize, target_bytes: usize) -> String {
    let unit = format!(
        "{needle}{}",
        "x".repeat(spacing.saturating_sub(needle.len()))
    );
    let reps = target_bytes / unit.len().max(1);
    unit.repeat(reps)
}

fn run_row(label: &str, text: &str, query: &str, replacement: &str) {
    let searcher = Searcher::new(query, SearchOptions::default()).expect("searcher");
    let count = searcher.count(&TextBuffer::from_str(text));
    assert!(count > 0, "fixture must contain matches");

    let t_string = time_iters(WARMUP, ITERS, || {
        let _ = string_replace_all(text, query, replacement);
    });
    let t_n_edits = time_iters(WARMUP, ITERS, || {
        let _ = n_edits_replace_all(text, query, replacement);
    });
    let t_opt = time_iters(WARMUP, ITERS, || {
        let _ = optimized_replace_all(text, &searcher, replacement);
    });

    let speedup = t_n_edits.as_secs_f64() / t_opt.as_secs_f64();
    println!(
        "{:<28} {:>9.2?} {:>9.2?} {:>9.2?} {:>7.1}x",
        format!("{label} ({count} hits)"),
        t_string,
        t_n_edits,
        t_opt,
        speedup
    );
}

fn time_iters(warmup: u32, iters: u32, mut f: impl FnMut()) -> Duration {
    for _ in 0..warmup {
        f();
    }
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    start.elapsed() / iters
}

/// Legacy `page.rs` style: count + `String::replace`.
fn string_replace_all(text: &str, query: &str, replacement: &str) -> usize {
    let n = text.matches(query).count();
    if n == 0 {
        return 0;
    }
    let _ = text.replace(query, replacement);
    n
}

/// Pre-optimization editor path: per-line `all_matches` + N rope edits + old slice each time.
fn n_edits_replace_all(text: &str, query: &str, template: &str) -> usize {
    let mut buf = TextBuffer::from_str(text);
    let pattern = format!("(?i){}", regex::escape(query));
    let re = Regex::new(&pattern).expect("pattern");
    let rope = buf.rope();
    let mut matches = Vec::new();
    for line in 0..rope.len_lines() {
        let line_start = rope.line_to_char(line);
        let body = line_body_slow(&buf, line);
        for m in re.find_iter(&body) {
            let start = line_start + byte_to_char(&body, m.start());
            let end = line_start + byte_to_char(&body, m.end());
            matches.push((start, end));
        }
    }
    if matches.is_empty() {
        return 0;
    }
    let count = matches.len();
    let mut edits = Vec::with_capacity(count);
    for (start, end) in &matches {
        let old: String = buf.rope().slice(*start..*end).chars().collect();
        edits.push(Edit::replace(*start, old, template.to_string()));
    }
    let txn = Transaction::new(edits, SelectionSet::default(), SelectionSet::default());
    let _ = txn.apply(&mut buf);
    count
}

/// Current production path after optimization.
fn optimized_replace_all(text: &str, searcher: &Searcher, template: &str) -> usize {
    let mut doc = Document::empty();
    doc.replace_range(0..0, text);
    doc.replace_all(searcher, template, false)
}

// --- helpers duplicated from pre-rope_scan `all_matches` for the legacy bench ---

fn line_body_slow(buffer: &TextBuffer, line: usize) -> String {
    buffer.line_text(line)
}

fn byte_to_char(s: &str, byte_idx: usize) -> usize {
    s[..byte_idx.min(s.len())].chars().count()
}
