//! cargo-nextest's `libtest-json-plus` to the per-test result file (plan A, JSON Lines v1; lane
//! akgorust). The stream is read from the run's stdout (it is on only when both
//! `NEXTEST_EXPERIMENTAL_LIBTEST_JSON=1` and `NEXTEST_MESSAGE_FORMAT=libtest-json-plus` are set, and
//! the person's own `--message-format` beats them: `kits::cargo_nextest`), nextest's human lines stay
//! on stderr, and the versions are not in it (`runner_version` is empty).
//!
//! What it writes, and when it writes nothing (no session line, the next reader takes over):
//! - no line is a libtest event: nothing. A build failure (cargo's diagnostics are on stderr), exit
//!   4 (no tests to run) and a run under the person's own `--message-format human` print no JSON.
//! - an event the converter does not know (a `test` event other than started, ok, failed, ignored,
//!   timeout; a `suite` event other than started, ok, failed): the format changed, nothing is read.
//! - a run that did not finish (a suite started and never ended), a test that started, never ended
//!   and was not ignored, a process that died by a signal (exit 128 + n or negative), a stream with
//!   `kept::cut`'s `[familiar: N chars dropped]` line in it (its middle is gone and what is left can
//!   look complete): the lines seen, and no `end` line, so nothing reads it. A stream cut exactly
//!   after nextest's extra first suite pair (see below) looks complete; the exit code is the guard.
//!
//! nextest 0.9.143 reports an ignored test as a test started with no end (its `#[ignore = ".."]`
//! reason is lost); the suites' `ignored` counts say how many may be left that way. A failed test is
//! its first panic: `thread 'x' panicked at file:line:col:` and the message after it (rust 1.73 on),
//! or `panicked at 'message', file:line:col` before; libtest has no exception type, so the kind is
//! `assertion` when the message starts with "assertion" (`assert!`, `assert_eq!`, `assert_ne!`) and
//! `exception` for anything else, a custom-message `assert!` and an `unwrap` among them. The one
//! frame is the panic site with no function name (there is no backtrace to say which), and a test's
//! `file` is set only when the panic is in its own `tests/<binary>.rs`.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::{json, Value};

/// `panicked at file:line:col:` and the message on the lines after it (rust 1.73 on).
fn panic_new() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)panicked at (\S+?):(\d+):(\d+):\n(.*)").expect("panic, new"))
}

/// `panicked at 'message', file:line:col` (before rust 1.73).
fn panic_old() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?s)panicked at '(.*?)', (\S+?):(\d+):(\d+)").expect("panic, old"))
}

/// `kept::cut`'s line where a stream's middle was dropped: what is left can look complete.
fn is_cut_mark(line: &str) -> bool {
    line.starts_with("[familiar: ") && line.ends_with(" chars dropped]")
}

/// A process that died by a signal (the shell's 128 + n, or negative) did not finish.
fn died(exit: i32) -> bool {
    !(0..128).contains(&exit)
}

#[derive(Default)]
struct Test {
    started: bool,
    end: Option<String>,
    stdout: String,
}

/// The panic in a failed test's captured output: (file, line, message).
fn panic_in(stdout: &str) -> Option<(String, u64, String)> {
    if let Some(m) = panic_new().captures(stdout) {
        // the message ends where rust's own note, or a backtrace, begins
        let rest = &m[4];
        let end = ["\nnote: run with", "\nstack backtrace:"].iter().filter_map(|s| rest.find(s)).min().unwrap_or(rest.len());
        return Some((m[1].to_string(), m[2].parse().unwrap_or(0), rest[..end].trim_end().to_string()));
    }
    let m = panic_old().captures(stdout)?;
    Some((m[2].to_string(), m[3].parse().unwrap_or(0), m[1].to_string()))
}

/// The test binary's name in `crate::binary$test`, `binary` alone for the crate's own tests.
fn test_binary(name: &str) -> &str {
    let binary = name.split('$').next().unwrap_or(name);
    binary.rsplit("::").next().unwrap_or(binary)
}

fn test_line(name: &str, t: &Test, outcome: &str) -> Value {
    let mut file = Value::Null;
    let mut error = Value::Null;
    if outcome == "failed" {
        error = match panic_in(&t.stdout) {
            Some((f, l, message)) => {
                let own = std::path::Path::new(&f).file_stem().is_some_and(|s| s == test_binary(name)) && (f.starts_with("tests/") || f.contains("/tests/"));
                if own {
                    file = json!(f);
                }
                let kind = if message.starts_with("assertion") { "assertion" } else { "exception" };
                json!({"type": "panic", "message": message, "kind": kind, "frames": [{"file": f, "line": l, "function": null}]})
            }
            None if t.end.as_deref() == Some("timeout") => json!({"type": "timeout", "message": "the test timed out", "kind": "exception", "frames": []}),
            None => {
                let tail: String = t.stdout.chars().rev().take(500).collect::<Vec<_>>().into_iter().rev().collect();
                json!({"type": "unknown", "message": tail, "kind": "exception", "frames": []})
            }
        };
    }
    json!({"type": "test", "id": name, "file": file, "line": null, "outcome": outcome, "skip_reason": null, "error": error})
}

/// The per-test result file for a nextest `libtest-json-plus` run's stdout and exit code, or the
/// empty string when the output holds no structured result (see the module's note).
pub fn to_report(native: &str, exit: i32) -> String {
    convert(native, exit).unwrap_or_default()
}

fn convert(native: &str, exit: i32) -> Option<String> {
    let mut order: Vec<String> = Vec::new();
    let mut tests: HashMap<String, Test> = HashMap::new();
    let (mut suites_started, mut suites_ended, mut ignored_total, mut filtered_total) = (0u64, 0u64, 0u64, 0u64);
    let mut saw_event = false;
    let mut cut = false;
    for line in native.lines() {
        let Ok(Value::Object(e)) = serde_json::from_str::<Value>(line) else {
            cut |= is_cut_mark(line);
            continue;
        };
        let event = e.get("event").and_then(Value::as_str);
        match (e.get("type").and_then(Value::as_str), event) {
            (Some("suite"), Some("started")) => suites_started += 1,
            (Some("suite"), Some("ok" | "failed")) => {
                suites_ended += 1;
                ignored_total += e.get("ignored").and_then(Value::as_u64).unwrap_or(0);
                filtered_total += e.get("filtered_out").and_then(Value::as_u64).unwrap_or(0);
            }
            (Some("test"), Some(ev @ ("started" | "ok" | "failed" | "ignored" | "timeout"))) => {
                let name = e.get("name").and_then(Value::as_str)?.to_string();
                if !tests.contains_key(&name) {
                    order.push(name.clone());
                }
                let t = tests.entry(name).or_default();
                if ev == "started" {
                    t.started = true;
                } else {
                    t.end = Some(ev.to_string());
                    // only a failure's output is read: a big run stays small
                    if matches!(ev, "failed" | "timeout") {
                        t.stdout = e.get("stdout").and_then(Value::as_str).unwrap_or("").to_string();
                    }
                }
            }
            // an event of a kind this converter does not know: the format changed
            (Some("suite" | "test"), _) => return None,
            _ => continue,
        }
        saw_event = true;
    }
    if !saw_event {
        return None;
    }

    // a test started with no end is an ignored one, as many as the suites say were ignored
    let unfinished = tests.values().filter(|t| t.end.is_none()).count() as u64;
    let finished = suites_started == suites_ended && unfinished <= ignored_total && !cut && !died(exit);

    let mut lines = vec![json!({"type": "session", "v": 1, "runner": "cargo-nextest", "runner_version": "", "language": "rust", "language_version": "", "plugins": []})];
    let mut counts = [0u64; 3]; // passed, failed, skipped
    for name in &order {
        let t = &tests[name];
        let outcome = match t.end.as_deref() {
            Some("ok") => "passed",
            Some("failed" | "timeout") => "failed",
            Some("ignored") => "skipped",
            _ if finished => "skipped",
            _ => "error",
        };
        match outcome {
            "passed" => counts[0] += 1,
            "failed" => counts[1] += 1,
            "skipped" => counts[2] += 1,
            _ => {}
        }
        lines.push(test_line(name, t, outcome));
    }
    if finished {
        lines.push(json!({"type": "end", "exit": exit, "counts": {"passed": counts[0], "failed": counts[1], "skipped": counts[2], "deselected": filtered_total, "errors": 0}}));
    }
    Some(lines.iter().map(|l| format!("{l}\n")).collect())
}
