//! node --test's TAP as the per-test result file (JSON Lines v1) [draft] (lane AKjs, plan of record
//! 2026-10-19, A).
//!
//! ```text
//!   TAP version 13                          {"type":"session",…}
//!   # Subtest: adds                         {"type":"test","id":"…::adds","outcome":"failed",
//!   not ok 1 - adds                          "error":{"type","message","kind","frames"}}
//!     ---  …  location: '/p/t.js:6:1'  …    {"type":"collect_error",…}        (a file that did not load)
//!     ...                                   {"type":"end","exit":<exit>,"counts":{…}}
//!   # tests 1  # pass 0  # fail 1 …
//! ```
//!
//! TAP names no file for a test that passed and carries no versions, so such a test has `file` and
//! `line` null, and the session's versions are null; a failing test has its file and line from
//! `location`, its type from `name`, its frames from `stack` (project frames only, outermost first,
//! deepest last). A file node could not load shows as node's own test named for the file at line 1
//! column 1: it becomes a `collect_error` built from the child's stderr, which node prints as `#`
//! comment lines before it. A file that ran no test shows as the same test passing: dropped.
//!
//! Never wrong: output that is not a complete TAP run (no header, no summary, a bail out) converts
//! to the empty string, which has no structured output.

use serde_json::{json, Value};

/// `to_report_in` with the paths left as TAP printed them (absolute).
pub fn to_report(native: &str, exit: i32) -> String {
    to_report_in(native, exit, "")
}

/// The run's TAP as JSON Lines, files relative to `root` (a directory; empty leaves paths as
/// printed). `exit` is the run's own exit code.
pub fn to_report_in(native: &str, exit: i32, root: &str) -> String {
    convert(native, exit, root).unwrap_or_default()
}

/// A result line: `ok 1 - name # SKIP why`, with the YAML block that follows it.
struct Line {
    ok: bool,
    name: String,
    directive: Option<(String, String)>,
    yaml: Vec<(String, String)>,
}

impl Line {
    fn get(&self, key: &str) -> Option<&str> {
        self.yaml.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

fn convert(native: &str, exit: i32, root: &str) -> Option<String> {
    let lines: Vec<&str> = native.lines().map(|l| l.trim_end_matches('\r')).collect();
    if !lines.iter().find(|l| !l.trim().is_empty())?.starts_with("TAP version") || lines.iter().any(|l| l.starts_with("Bail out!")) {
        return None;
    }
    let summary = |key: &str| lines.iter().find_map(|l| l.strip_prefix(&format!("# {key} ")).and_then(|n| n.trim().parse::<u64>().ok()));
    summary("tests")?;
    summary("pass")?;
    summary("fail")?;

    let mut out: Vec<Value> = vec![json!({"type": "session", "v": 1, "runner": "node-test", "runner_version": null, "language": "javascript", "language_version": null, "plugins": []})];
    let (mut passed, mut failed, mut skipped, mut errors) = (0u64, 0u64, 0u64, 0u64);
    let mut open: Vec<String> = Vec::new();
    let mut stderr: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        let indent = line.len() - line.trim_start().len();
        let text = line.trim_start();
        if let Some(name) = text.strip_prefix("# Subtest: ") {
            open.truncate(indent / 4);
            open.push(unescape(name));
        } else if indent == 0 && text.starts_with("# ") && !is_summary(text) {
            stderr.push(text[2..].to_string());
        } else if let Some(mut r) = result(text) {
            if lines.get(i).is_some_and(|l| l.trim_end() == format!("{}---", " ".repeat(indent + 2))) {
                let (yaml, used) = yaml_block(&lines[i + 1..], indent + 2);
                r.yaml = yaml;
                i += 1 + used;
            }
            let comments = std::mem::take(&mut stderr);
            if r.get("type") == Some("suite") {
                // a suite fails for its own hook (hookFailed, once, on the suite; its tests are only
                // cancelled) or because a test in it failed (those tests say so themselves)
                if !r.ok && r.get("failureType") == Some("hookFailed") {
                    let (file, at, _) = location(r.get("location").unwrap_or(""));
                    let rel = file.as_deref().map(|f| relative(f, root));
                    let names: Vec<&str> = open.iter().take(indent / 4).map(String::as_str).chain(std::iter::once(r.name.as_str())).collect();
                    failed += 1;
                    out.push(json!({"type": "test", "id": format!("{}::{} (hook)", rel.as_deref().unwrap_or(""), names.join(" > ")), "file": rel, "line": at, "outcome": "failed", "skip_reason": null, "error": failure(&r, root)}));
                }
                continue;
            }
            let (file, at, col) = location(r.get("location").unwrap_or(""));
            let rel = file.as_deref().map(|f| relative(f, root));
            // node's own test for a file: named for the file, at line 1 column 1 when it failed (a
            // file that did not load); when it passed (a file with no test) TAP gives no location, so
            // the name that is a test file's path is what is left to know it by
            let synthetic = indent == 0
                && match &file {
                    Some(f) => at == Some(1) && col == Some(1) && (r.name == relative(f, root) || f.ends_with(r.name.as_str()) || r.name == *f),
                    None => r.ok && r.directive.is_none() && is_test_file_path(&r.name),
                };
            if synthetic {
                if !r.ok {
                    errors += 1;
                    out.push(json!({"type": "collect_error", "file": rel, "error": collect_error(&comments, &r, root)}));
                }
                continue;
            }
            let names: Vec<&str> = open.iter().take(indent / 4).map(String::as_str).chain(std::iter::once(r.name.as_str())).collect();
            let id = format!("{}::{}", rel.as_deref().unwrap_or(""), names.join(" > "));
            let (outcome, reason, error) = match (&r.directive, r.ok) {
                (Some((_, why)), _) => {
                    skipped += 1;
                    ("skipped", (!why.is_empty()).then(|| why.clone()), Value::Null)
                }
                (None, true) => {
                    passed += 1;
                    ("passed", None, Value::Null)
                }
                (None, false) => {
                    failed += 1;
                    ("failed", None, failure(&r, root))
                }
            };
            out.push(json!({"type": "test", "id": id, "file": rel, "line": at, "outcome": outcome, "skip_reason": reason, "error": error}));
        }
    }
    out.push(json!({"type": "end", "exit": exit, "counts": {"passed": passed, "failed": failed, "skipped": skipped, "deselected": 0, "errors": errors}}));
    Some(out.iter().map(|v| v.to_string() + "\n").collect())
}

fn is_summary(text: &str) -> bool {
    ["tests", "suites", "pass", "fail", "cancelled", "skipped", "todo", "duration_ms"].iter().any(|k| text.strip_prefix(&format!("# {k} ")).is_some_and(|n| n.trim().parse::<f64>().is_ok()))
}

fn is_test_file_path(name: &str) -> bool {
    !name.contains(' ') && [".js", ".mjs", ".cjs", ".ts", ".mts", ".cts", ".jsx", ".tsx"].iter().any(|e| name.ends_with(e))
}

fn unescape(s: &str) -> String {
    s.replace("\\#", "#").replace("\\\\", "\\")
}

/// `ok 1 - name # SKIP why` / `not ok 2 - name`.
fn result(text: &str) -> Option<Line> {
    let (ok, rest) = match text.strip_prefix("not ok ") {
        Some(r) => (false, r),
        None => (true, text.strip_prefix("ok ")?),
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = rest[digits..].strip_prefix(" - ").or_else(|| (rest.len() == digits).then_some("")).unwrap_or(&rest[digits..]);
    let (name, directive) = match rest.split_once(" # ") {
        Some((n, d)) => {
            let (kind, why) = d.split_once(' ').unwrap_or((d, ""));
            (n, matches!(kind, "SKIP" | "TODO").then(|| (kind.to_string(), why.trim().to_string())))
        }
        None => (rest, None),
    };
    Some(Line { ok, name: unescape(name), directive, yaml: Vec::new() })
}

/// The `key: value` pairs of the YAML block at `base` spaces, and how many lines it took (the
/// closing `...` included). A `|-` value is the lines indented deeper, de-indented.
fn yaml_block(lines: &[&str], base: usize) -> (Vec<(String, String)>, usize) {
    let end = format!("{}...", " ".repeat(base));
    let mut pairs = Vec::new();
    let mut i = 0;
    while i < lines.len() && lines[i].trim_end() != end {
        let l = lines[i];
        i += 1;
        let indent = l.len() - l.trim_start().len();
        if indent != base {
            continue;
        }
        let Some((key, value)) = l.trim_start().split_once(": ").or_else(|| l.trim_start().strip_suffix(':').map(|k| (k, ""))) else { continue };
        if value.starts_with('|') || value.starts_with('>') {
            let mut block: Vec<&str> = Vec::new();
            while i < lines.len() && lines[i].trim_end() != end && (lines[i].trim().is_empty() || lines[i].len() - lines[i].trim_start().len() > base) {
                block.push(lines[i].get(base + 2..).unwrap_or(""));
                i += 1;
            }
            while block.last().is_some_and(|b| b.trim().is_empty()) {
                block.pop();
            }
            pairs.push((key.to_string(), block.join("\n")));
        } else {
            pairs.push((key.to_string(), value.trim().trim_matches('\'').replace("\\'", "'").replace("''", "'")));
        }
    }
    (pairs, i + usize::from(i < lines.len()))
}

/// `'/p/t.js:6:1'` as (file, line, column).
fn location(s: &str) -> (Option<String>, Option<u64>, Option<u64>) {
    let mut parts = s.rsplitn(3, ':');
    let (col, line, file) = (parts.next(), parts.next(), parts.next());
    match (file, line.and_then(|l| l.parse().ok()), col.and_then(|c| c.parse().ok())) {
        (Some(f), l @ Some(_), c @ Some(_)) if !f.is_empty() => (Some(f.to_string()), l, c),
        _ => (None, None, None),
    }
}

fn relative(path: &str, root: &str) -> String {
    let root = root.trim_end_matches('/');
    match path.strip_prefix(root) {
        Some(rest) if !root.is_empty() && rest.starts_with('/') => rest[1..].to_string(),
        _ => path.to_string(),
    }
}

/// The kit's own reading of an error: a missing name, an assertion, an exception.
fn kind(name: &str, code: &str, message: &str) -> &'static str {
    if [" is not a function", "is not defined", "does not provide an export named", " is not a constructor"].iter().any(|m| message.contains(m)) {
        "missing_name"
    } else if name.contains("AssertionError") || code == "ERR_ASSERTION" || message.trim_start().starts_with("expect(") {
        "assertion"
    } else {
        "exception"
    }
}

/// Project frames of a stack, outermost first: `fn (/p/file.js:7:10)` or a bare `/p/file.js:7:10`.
fn frames<'a>(stack: impl Iterator<Item = &'a str>, root: &str) -> Vec<Value> {
    let mut out: Vec<Value> = stack
        .filter_map(|l| {
            let t = l.trim().trim_start_matches("at ").trim();
            let (function, at) = match (t.rfind(" ("), t.strip_suffix(')')) {
                (Some(i), Some(inner)) => (Some(t[..i].trim_start_matches("async ").trim_start_matches("new ").to_string()), &inner[i + 2..]),
                _ => (None, t),
            };
            let (file, line, _) = location(at);
            let file = file?;
            let own = !file.starts_with("node:") && !file.starts_with("internal/") && !file.contains("node_modules") && if root.is_empty() { file.starts_with('/') } else { relative(&file, root) != file };
            own.then(|| json!({"file": relative(&file, root), "line": line, "function": function}))
        })
        .collect();
    out.reverse();
    out
}

fn failure(r: &Line, root: &str) -> Value {
    let message: String = r.get("error").unwrap_or("").chars().take(500).collect();
    let name = r.get("name").unwrap_or("");
    let kind = if r.get("failureType") == Some("hookFailed") { "setup" } else { kind(name, r.get("code").unwrap_or(""), &message) };
    json!({"type": (!name.is_empty()).then_some(name), "message": message, "kind": kind, "frames": frames(r.get("stack").unwrap_or("").lines(), root)})
}

/// A file that did not load: node printed the child's stderr as `#` comments before the test named
/// for the file; its first `SomethingError: message` line and the `at` lines after it.
fn collect_error(comments: &[String], r: &Line, root: &str) -> Value {
    let found = comments.iter().enumerate().find_map(|(at, c)| {
        let (name, message) = c.split_once(": ")?;
        let name = name.split(" [").next()?;
        (!name.is_empty() && name.ends_with("Error") && name.chars().all(|c| c.is_ascii_alphanumeric())).then(|| (at, name.to_string(), message.to_string()))
    });
    match found {
        Some((at, name, message)) => {
            let code = comments[at..].iter().find_map(|c| c.trim().strip_prefix("code: ")).unwrap_or("").trim_matches(|c| c == '\'' || c == ',');
            let stack = comments[at + 1..].iter().map(String::as_str).filter(|c| c.trim_start().starts_with("at "));
            let kind = if message.contains("Cannot find module") || code == "MODULE_NOT_FOUND" { "exception" } else { kind(&name, "", &message) };
            json!({"type": name, "message": message.chars().take(500).collect::<String>(), "kind": kind, "frames": frames(stack, root)})
        }
        None => json!({"type": null, "message": r.get("error").unwrap_or("test failed"), "kind": "exception", "frames": []}),
    }
}
